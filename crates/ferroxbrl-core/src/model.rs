use serde::Serialize;
use std::collections::BTreeMap;

/// How the fact was expressed in the inline document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    NonFraction,
    NonNumeric,
    Hidden,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fact {
    /// Prefixed concept name as written, e.g. "us-gaap:Revenues".
    pub concept: String,
    /// Resolved namespace URI of the concept.
    pub concept_ns: String,
    /// Local part of the concept name.
    pub concept_local: String,
    pub kind: FactKind,
    pub context_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_ref: Option<String>,
    /// Canonical value after transform/sign/scale. None when xsi:nil.
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// nonNumeric escape="true": value contains markup which we capture as text.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub escaped: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Period {
    Instant { instant: String },
    Duration { start: String, end: String },
    Forever { forever: bool },
}

#[derive(Debug, Clone, Serialize)]
pub struct Context {
    pub id: String,
    pub entity_scheme: String,
    pub entity: String,
    pub period: Period,
    /// dimension (prefixed) -> member (prefixed). Typed members are stored
    /// as "(typed) <text>" and reported in warnings.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub dims: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Unit {
    pub id: String,
    pub numerators: Vec<String>,
    pub denominators: Vec<String>,
}

impl Unit {
    /// Canonical display form, e.g. "iso4217:USD" or "iso4217:USD/xbrli:shares".
    pub fn display(&self) -> String {
        let num = self.numerators.join("*");
        if self.denominators.is_empty() {
            num
        } else {
            format!("{}/{}", num, self.denominators.join("*"))
        }
    }
}

/// Parse result for one inline XBRL document.
#[derive(Debug, Default)]
pub struct Document {
    pub facts: Vec<Fact>,
    pub contexts: BTreeMap<String, Context>,
    pub units: BTreeMap<String, Unit>,
    pub warnings: Vec<String>,
}

impl Document {
    /// Facts joined with their context and unit, ready for JSON output.
    pub fn flat_facts(&self) -> Vec<serde_json::Value> {
        self.facts
            .iter()
            .map(|f| {
                let ctx = self.contexts.get(&f.context_ref);
                let unit = f
                    .unit_ref
                    .as_ref()
                    .and_then(|u| self.units.get(u))
                    .map(|u| u.display());
                serde_json::json!({
                    "concept": f.concept,
                    "concept_ns": f.concept_ns,
                    "kind": f.kind,
                    "value": f.value,
                    "decimals": f.decimals,
                    "unit": unit,
                    "entity": ctx.map(|c| c.entity.clone()),
                    "period": ctx.map(|c| serde_json::to_value(&c.period).unwrap()),
                    "dims": ctx.map(|c| c.dims.clone()),
                    "context_ref": f.context_ref,
                })
            })
            .collect()
    }
}
