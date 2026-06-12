//! Single-pass streaming parser for inline XBRL (iXBRL 1.0/1.1) documents.
//!
//! Design: one forward pass over the XHTML keeping (a) a namespace stack we
//! manage ourselves (QName attribute *values* like name="us-gaap:Foo" need
//! in-scope prefix resolution that XML readers don't expose), (b) open fact
//! capture frames for ix:nonFraction / ix:nonNumeric / hidden facts, and
//! (c) context/unit builders for the xbrli resources embedded in ix:header.
//! Continuations (continuedAt chains) resolve after the pass.
//!
//! Known v0 limitations (tracked for the Arelle-diff harness to quantify):
//! - escape="true" nonNumeric facts capture text content only (markup dropped)
//! - nested ix facts contribute their text to enclosing facts unconditionally
//! - ix:tuple / ix:fraction / ix:footnote are skipped with a warning
//! - typed dimension members are captured by their text value (nested complex
//!   typed members are not modelled)

use crate::model::{Context, Document, Fact, FactKind, Period, Unit};
use crate::transform::{self, Transformed};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::{BTreeMap, HashMap, HashSet};

const IX_NS_11: &str = "http://www.xbrl.org/2013/inlineXBRL";
const IX_NS_10: &str = "http://www.xbrl.org/2008/inlineXBRL";
const XBRLI_NS: &str = "http://www.xbrl.org/2003/instance";
const XBRLDI_NS: &str = "http://xbrl.org/2006/xbrldi";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const LINK_NS: &str = "http://www.xbrl.org/2003/linkbase";

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Error {}

/// An in-progress ix fact capture.
struct Capture {
    kind: FactKind,
    concept_raw: String,
    concept_ns: String,
    context_ref: String,
    unit_ref: Option<String>,
    scale: i32,
    sign_minus: bool,
    decimals: Option<String>,
    format: Option<String>,
    escaped: bool,
    nil: bool,
    continued_at: Option<String>,
    text: String,
    exclude_depth_at_open: usize,
    depth_at_open: usize,
}

/// An ix:continuation being captured.
struct Continuation {
    text: String,
    continued_at: Option<String>,
    exclude_depth_at_open: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum CtxLeaf {
    Identifier,
    Instant,
    StartDate,
    EndDate,
    ExplicitMember,
    TypedMember,
}

struct CtxBuilder {
    id: String,
    depth_at_open: usize,
    entity_scheme: String,
    entity: String,
    instant: Option<String>,
    start: Option<String>,
    end: Option<String>,
    forever: bool,
    dims: BTreeMap<String, String>,
    cur_leaf: Option<CtxLeaf>,
    cur_dim: Option<String>,
    leaf_text: String,
}

struct UnitBuilder {
    id: String,
    depth_at_open: usize,
    numerators: Vec<String>,
    denominators: Vec<String>,
    in_denominator: bool,
    denom_depth: usize,
    in_measure: bool,
    measure_text: String,
}

/// Raw value + numeric parameters held until continuations resolve.
struct PendingValue {
    text: String,
    scale: i32,
    sign_minus: bool,
    nil: bool,
    kind: FactKind,
    format: Option<String>,
    continued_at: Option<String>,
}

pub struct Parser {
    ns_stack: Vec<Vec<(String, String)>>,
    depth: usize,
    exclude_depth: usize,
    exclude_depths: Vec<usize>,
    hidden_depth: Option<usize>,
    captures: Vec<Capture>,
    continuations: HashMap<String, Continuation>,
    open_continuations: Vec<(String, usize)>,
    ctx: Option<CtxBuilder>,
    unit: Option<UnitBuilder>,
    doc: Document,
    pending_facts: Vec<Fact>,
    pending_values: Vec<PendingValue>,
    warned: HashSet<String>,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            ns_stack: Vec::with_capacity(64),
            depth: 0,
            exclude_depth: 0,
            exclude_depths: Vec::new(),
            hidden_depth: None,
            captures: Vec::new(),
            continuations: HashMap::new(),
            open_continuations: Vec::new(),
            ctx: None,
            unit: None,
            doc: Document::default(),
            pending_facts: Vec::new(),
            pending_values: Vec::new(),
            warned: HashSet::new(),
        }
    }

    pub fn parse(mut self, xml: &str) -> Result<Document, Error> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().check_end_names = false;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => self.on_start(&e, false),
                Ok(Event::Empty(e)) => self.on_start(&e, true),
                Ok(Event::End(_)) => self.on_end(),
                Ok(Event::Text(t)) => {
                    match t.unescape_with(resolve_html_entity) {
                        Ok(c) => {
                            let owned = c.into_owned();
                            self.on_text(&owned);
                        }
                        Err(_) => {
                            let owned = String::from_utf8_lossy(t.as_ref()).into_owned();
                            self.on_text(&owned);
                        }
                    }
                }
                Ok(Event::CData(t)) => {
                    let owned = String::from_utf8_lossy(t.as_ref()).into_owned();
                    self.on_text(&owned);
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(Error(format!(
                        "XML parse error at byte {}: {e}",
                        reader.buffer_position()
                    )))
                }
            }
        }
        self.finish()
    }

    // ---- namespaces ---------------------------------------------------------

    fn push_ns_scope(&mut self, e: &BytesStart) {
        let mut decls: Vec<(String, String)> = Vec::new();
        for attr in e.attributes().with_checks(false).flatten() {
            let key = attr.key.as_ref();
            if key == b"xmlns" {
                decls.push((String::new(), attr_value(&attr)));
            } else if let Some(p) = key.strip_prefix(b"xmlns:") {
                decls.push((String::from_utf8_lossy(p).into_owned(), attr_value(&attr)));
            }
        }
        self.ns_stack.push(decls);
    }

    fn resolve_prefix(&self, prefix: &str) -> Option<&str> {
        for scope in self.ns_stack.iter().rev() {
            for (p, uri) in scope.iter().rev() {
                if p == prefix {
                    return Some(uri.as_str());
                }
            }
        }
        None
    }

    /// Resolve "prefix:local" (or "local") to the in-scope namespace URI.
    fn resolve_qname_ns(&self, qname: &str) -> String {
        match qname.split_once(':') {
            Some((p, _)) => self.resolve_prefix(p).unwrap_or("").to_string(),
            None => self.resolve_prefix("").unwrap_or("").to_string(),
        }
    }

    // ---- start / end ----------------------------------------------------------

    fn on_start(&mut self, e: &BytesStart, is_empty: bool) {
        self.push_ns_scope(e);
        self.depth += 1;

        let raw_name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        let ns = self.resolve_qname_ns(&raw_name);
        let local = raw_name.rsplit(':').next().unwrap_or(&raw_name).to_string();

        let is_ix = ns == IX_NS_11 || ns == IX_NS_10;
        if is_ix {
            match local.as_str() {
                "nonFraction" | "nonNumeric" => self.start_ix_fact(e, &local),
                "continuation" => {
                    if let Some(id) = get_attr(e, b"id") {
                        let continued_at = get_attr(e, b"continuedAt");
                        self.continuations.insert(
                            id.clone(),
                            Continuation {
                                text: String::new(),
                                continued_at,
                                exclude_depth_at_open: self.exclude_depth,
                            },
                        );
                        self.open_continuations.push((id, self.depth));
                    }
                }
                "exclude" => {
                    self.exclude_depth += 1;
                    self.exclude_depths.push(self.depth);
                }
                "hidden" => self.hidden_depth = Some(self.depth),
                "tuple" | "fraction" | "numerator" | "denominator" | "footnote" => {
                    self.warn_once(&format!("ix:{local} not supported yet (skipped)"));
                }
                _ => {}
            }
        } else if ns == XBRLI_NS {
            match local.as_str() {
                "context" => {
                    self.ctx = Some(CtxBuilder {
                        id: get_attr(e, b"id").unwrap_or_default(),
                        depth_at_open: self.depth,
                        entity_scheme: String::new(),
                        entity: String::new(),
                        instant: None,
                        start: None,
                        end: None,
                        forever: false,
                        dims: BTreeMap::new(),
                        cur_leaf: None,
                        cur_dim: None,
                        leaf_text: String::new(),
                    });
                }
                "identifier" => {
                    if let Some(c) = self.ctx.as_mut() {
                        c.entity_scheme = get_attr(e, b"scheme").unwrap_or_default();
                        c.cur_leaf = Some(CtxLeaf::Identifier);
                        c.leaf_text.clear();
                    }
                }
                "instant" => self.set_ctx_leaf(CtxLeaf::Instant),
                "startDate" => self.set_ctx_leaf(CtxLeaf::StartDate),
                "endDate" => self.set_ctx_leaf(CtxLeaf::EndDate),
                "forever" => {
                    if let Some(c) = self.ctx.as_mut() {
                        c.forever = true;
                    }
                }
                "unit" => {
                    self.unit = Some(UnitBuilder {
                        id: get_attr(e, b"id").unwrap_or_default(),
                        depth_at_open: self.depth,
                        numerators: Vec::new(),
                        denominators: Vec::new(),
                        in_denominator: false,
                        denom_depth: 0,
                        in_measure: false,
                        measure_text: String::new(),
                    });
                }
                "unitDenominator" => {
                    if let Some(u) = self.unit.as_mut() {
                        u.in_denominator = true;
                        u.denom_depth = self.depth;
                    }
                }
                "measure" => {
                    if let Some(u) = self.unit.as_mut() {
                        u.in_measure = true;
                        u.measure_text.clear();
                    }
                }
                _ => {}
            }
        } else if ns == XBRLDI_NS {
            match local.as_str() {
                "explicitMember" => {
                    let dim = get_attr(e, b"dimension").unwrap_or_default();
                    if let Some(c) = self.ctx.as_mut() {
                        c.cur_dim = Some(dim);
                        c.cur_leaf = Some(CtxLeaf::ExplicitMember);
                        c.leaf_text.clear();
                    }
                }
                "typedMember" => {
                    let dim = get_attr(e, b"dimension").unwrap_or_default();
                    if let Some(c) = self.ctx.as_mut() {
                        c.cur_dim = Some(dim);
                        c.cur_leaf = Some(CtxLeaf::TypedMember);
                        c.leaf_text.clear();
                    }
                    self.warn_once("typedMember captured as text (full typed dimension support pending)");
                }
                _ => {}
            }
        } else if self.hidden_depth.is_some()
            && self.ctx.is_none()
            && self.unit.is_none()
            && ns != LINK_NS
            && !ns.is_empty()
        {
            // A raw XBRL fact inside ix:hidden, e.g. <dei:Foo contextRef="..">v</..>
            if let Some(context_ref) = get_attr(e, b"contextRef") {
                let nil = self.get_xsi_nil(e);
                self.captures.push(Capture {
                    kind: FactKind::Hidden,
                    concept_ns: ns.clone(),
                    concept_raw: raw_name.clone(),
                    context_ref,
                    unit_ref: get_attr(e, b"unitRef"),
                    scale: 0,
                    sign_minus: false,
                    decimals: get_attr(e, b"decimals"),
                    format: None,
                    escaped: false,
                    nil,
                    continued_at: None,
                    text: String::new(),
                    exclude_depth_at_open: self.exclude_depth,
                    depth_at_open: self.depth,
                });
            }
        }

        if is_empty {
            self.on_end();
        }
    }

    fn start_ix_fact(&mut self, e: &BytesStart, local: &str) {
        let kind = if local == "nonFraction" {
            FactKind::NonFraction
        } else {
            FactKind::NonNumeric
        };
        let name = get_attr(e, b"name").unwrap_or_default();
        let concept_ns = self.resolve_qname_ns(&name);
        let nil = self.get_xsi_nil(e);
        let escape = get_attr(e, b"escape")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        self.captures.push(Capture {
            kind,
            concept_raw: name,
            concept_ns,
            context_ref: get_attr(e, b"contextRef").unwrap_or_default(),
            unit_ref: get_attr(e, b"unitRef"),
            scale: get_attr(e, b"scale").and_then(|s| s.trim().parse().ok()).unwrap_or(0),
            sign_minus: get_attr(e, b"sign").as_deref() == Some("-"),
            decimals: get_attr(e, b"decimals"),
            format: get_attr(e, b"format"),
            escaped: escape,
            nil,
            continued_at: get_attr(e, b"continuedAt"),
            text: String::new(),
            exclude_depth_at_open: self.exclude_depth,
            depth_at_open: self.depth,
        });
    }

    fn set_ctx_leaf(&mut self, leaf: CtxLeaf) {
        if let Some(c) = self.ctx.as_mut() {
            c.cur_leaf = Some(leaf);
            c.leaf_text.clear();
        }
    }

    fn on_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(c) = self.ctx.as_mut() {
            if c.cur_leaf.is_some() {
                c.leaf_text.push_str(text);
            }
            return;
        }
        if let Some(u) = self.unit.as_mut() {
            if u.in_measure {
                u.measure_text.push_str(text);
            }
            return;
        }
        let exclude_depth = self.exclude_depth;
        for cap in self.captures.iter_mut() {
            if exclude_depth <= cap.exclude_depth_at_open {
                cap.text.push_str(text);
            }
        }
        for (id, _) in self.open_continuations.iter() {
            if let Some(cont) = self.continuations.get_mut(id) {
                if exclude_depth <= cont.exclude_depth_at_open {
                    cont.text.push_str(text);
                }
            }
        }
    }

    fn on_end(&mut self) {
        // Close any fact capture opened at this depth.
        if let Some(pos) = self
            .captures
            .iter()
            .rposition(|c| c.depth_at_open == self.depth)
        {
            let cap = self.captures.remove(pos);
            self.finalize_capture(cap);
        }
        if let Some(pos) = self
            .open_continuations
            .iter()
            .rposition(|(_, d)| *d == self.depth)
        {
            self.open_continuations.remove(pos);
        }
        if self.hidden_depth == Some(self.depth) {
            self.hidden_depth = None;
        }
        if self.exclude_depths.last() == Some(&self.depth) {
            self.exclude_depths.pop();
            self.exclude_depth -= 1;
        }

        // Context: close leaves first, then the context element itself.
        let mut close_ctx = false;
        if let Some(c) = self.ctx.as_mut() {
            if let Some(leaf) = c.cur_leaf.take() {
                let text = transform::collapse_ws(&c.leaf_text);
                match leaf {
                    CtxLeaf::Identifier => c.entity = text,
                    CtxLeaf::Instant => c.instant = Some(text),
                    CtxLeaf::StartDate => c.start = Some(text),
                    CtxLeaf::EndDate => c.end = Some(text),
                    CtxLeaf::ExplicitMember => {
                        if let Some(d) = c.cur_dim.take() {
                            c.dims.insert(d, text);
                        }
                    }
                    CtxLeaf::TypedMember => {
                        // OIM represents typed members by their text value.
                        if let Some(d) = c.cur_dim.take() {
                            c.dims.insert(d, text);
                        }
                    }
                }
                c.leaf_text.clear();
            } else if c.depth_at_open == self.depth {
                close_ctx = true;
            }
        }
        if close_ctx {
            let c = self.ctx.take().unwrap();
            let period = if c.forever {
                Period::Forever { forever: true }
            } else if let Some(i) = c.instant {
                Period::Instant { instant: i }
            } else if let (Some(s), Some(e)) = (c.start, c.end) {
                Period::Duration { start: s, end: e }
            } else {
                self.doc
                    .warnings
                    .push(format!("context {} has no usable period", c.id));
                Period::Forever { forever: true }
            };
            self.doc.contexts.insert(
                c.id.clone(),
                Context {
                    id: c.id,
                    entity_scheme: c.entity_scheme,
                    entity: c.entity,
                    period,
                    dims: c.dims,
                },
            );
        }

        // Unit: measure leaf, denominator wrapper, then the unit element.
        let mut close_unit = false;
        if let Some(u) = self.unit.as_mut() {
            if u.in_measure {
                let m = transform::collapse_ws(&u.measure_text);
                if !m.is_empty() {
                    if u.in_denominator {
                        u.denominators.push(m);
                    } else {
                        u.numerators.push(m);
                    }
                }
                u.in_measure = false;
            } else if u.in_denominator && u.denom_depth == self.depth {
                u.in_denominator = false;
            } else if u.depth_at_open == self.depth {
                close_unit = true;
            }
        }
        if close_unit {
            let u = self.unit.take().unwrap();
            self.doc.units.insert(
                u.id.clone(),
                Unit {
                    id: u.id,
                    numerators: u.numerators,
                    denominators: u.denominators,
                },
            );
        }

        self.depth -= 1;
        self.ns_stack.pop();
    }

    fn finalize_capture(&mut self, cap: Capture) {
        self.pending_facts.push(Fact {
            concept: cap.concept_raw.clone(),
            concept_ns: cap.concept_ns,
            concept_local: cap
                .concept_raw
                .rsplit(':')
                .next()
                .unwrap_or(&cap.concept_raw)
                .to_string(),
            kind: cap.kind,
            context_ref: cap.context_ref,
            unit_ref: cap.unit_ref,
            value: None,
            decimals: cap.decimals,
            format: cap.format.clone(),
            escaped: cap.escaped,
        });
        self.pending_values.push(PendingValue {
            text: cap.text,
            scale: cap.scale,
            sign_minus: cap.sign_minus,
            nil: cap.nil,
            kind: cap.kind,
            format: cap.format,
            continued_at: cap.continued_at,
        });
    }

    fn warn_once(&mut self, msg: &str) {
        if self.warned.insert(msg.to_string()) {
            self.doc.warnings.push(msg.to_string());
        }
    }

    fn finish(mut self) -> Result<Document, Error> {
        let facts = std::mem::take(&mut self.pending_facts);
        let values = std::mem::take(&mut self.pending_values);
        for (mut fact, pv) in facts.into_iter().zip(values.into_iter()) {
            if pv.nil {
                fact.value = None;
                self.doc.facts.push(fact);
                continue;
            }
            let mut full = pv.text;
            let mut next = pv.continued_at;
            let mut visited: HashSet<String> = HashSet::new();
            while let Some(id) = next {
                if !visited.insert(id.clone()) {
                    self.doc.warnings.push(format!("continuation cycle at id {id}"));
                    break;
                }
                match self.continuations.get(&id) {
                    Some(c) => {
                        full.push_str(&c.text);
                        next = c.continued_at.clone();
                    }
                    None => {
                        self.doc.warnings.push(format!("missing continuation id {id}"));
                        break;
                    }
                }
            }
            fact.value =
                Some(self.compute_value(&full, pv.scale, pv.sign_minus, pv.kind, pv.format.as_deref()));
            self.doc.facts.push(fact);
        }
        Ok(self.doc)
    }

    fn compute_value(
        &mut self,
        raw: &str,
        scale: i32,
        sign_minus: bool,
        kind: FactKind,
        format: Option<&str>,
    ) -> String {
        let collapsed = transform::collapse_ws(raw);
        let transformed: Option<String> = match format {
            Some(f) => match transform::apply(f, &collapsed) {
                Transformed::Ok(v) => Some(v),
                Transformed::Invalid(e) => {
                    self.doc.warnings.push(format!("transform {f}: {e}"));
                    None
                }
                Transformed::Unknown => {
                    self.warn_once(&format!("unknown transform {f}"));
                    None
                }
            },
            None => None,
        };
        match kind {
            FactKind::NonFraction | FactKind::Hidden => {
                let base = match transformed {
                    Some(v) => v,
                    None => match transform::fallback_numeric(&collapsed) {
                        Transformed::Ok(v) => v,
                        _ => return collapsed,
                    },
                };
                if base.is_empty() {
                    return base;
                }
                let numeric = base.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-');
                let scaled = if numeric {
                    transform::shift_decimal(&base, scale)
                } else {
                    base
                };
                if sign_minus && numeric && !scaled.starts_with('-') {
                    format!("-{scaled}")
                } else {
                    scaled
                }
            }
            FactKind::NonNumeric => transformed.unwrap_or(collapsed),
        }
    }

    fn get_xsi_nil(&self, e: &BytesStart) -> bool {
        for attr in e.attributes().with_checks(false).flatten() {
            let key = attr.key.as_ref();
            if let Some(pos) = key.iter().position(|b| *b == b':') {
                let (prefix, local) = (&key[..pos], &key[pos + 1..]);
                if local == b"nil"
                    && self.resolve_prefix(&String::from_utf8_lossy(prefix)) == Some(XSI_NS)
                {
                    return attr_value(&attr) == "true";
                }
            }
        }
        false
    }
}

fn attr_value(attr: &quick_xml::events::attributes::Attribute) -> String {
    attr.unescape_value()
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(&attr.value).into_owned())
}

fn get_attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    for attr in e.attributes().with_checks(false).flatten() {
        if attr.key.as_ref() == name {
            return Some(attr_value(&attr));
        }
    }
    None
}

fn resolve_html_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "nbsp" => "\u{a0}",
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "rsquo" => "\u{2019}",
        "lsquo" => "\u{2018}",
        "ldquo" => "\u{201c}",
        "rdquo" => "\u{201d}",
        "sect" => "\u{a7}",
        "para" => "\u{b6}",
        "copy" => "\u{a9}",
        "reg" => "\u{ae}",
        "trade" => "\u{2122}",
        "bull" => "\u{2022}",
        "hellip" => "\u{2026}",
        "cent" => "\u{a2}",
        "pound" => "\u{a3}",
        "yen" => "\u{a5}",
        "euro" => "\u{20ac}",
        "deg" => "\u{b0}",
        "plusmn" => "\u{b1}",
        "frac12" => "\u{bd}",
        "times" => "\u{d7}",
        "divide" => "\u{f7}",
        _ => return None,
    })
}
