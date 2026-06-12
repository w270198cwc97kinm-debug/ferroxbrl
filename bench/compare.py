#!/usr/bin/env python3
"""Compare ferroxbrl fact extraction against Arelle's OIM (xBRL-JSON) export.

Usage: compare.py <ferrox.json> <arelle-oim.json>

Normalizes both fact sets to comparable keys and reports:
  - coverage: facts found by Arelle but missed by ferroxbrl (and vice versa)
  - numeric value match rate (the headline metric for kill criterion T1)
  - text value match rate (informational; escaped-HTML facts differ by design)
"""
import json
import sys
from collections import Counter
from datetime import datetime, timedelta
from decimal import Decimal, InvalidOperation

CORE_DIMS = {"concept", "entity", "period", "unit", "language", "noteId"}


def norm_entity(v):
    if not v:
        return ""
    return str(v).rsplit(":", 1)[-1].lstrip("0") or "0"


def _shift_back(dt_str):
    """OIM uses midnight-of-next-day for XBRL end-of-day semantics."""
    try:
        dt = datetime.fromisoformat(dt_str)
    except ValueError:
        return dt_str
    if dt.time() == datetime.min.time():
        return (dt - timedelta(days=1)).date().isoformat()
    return dt_str


def norm_period_oim(v):
    if not v:
        return "forever"
    if "/" in v:
        start, end = v.split("/", 1)
        try:
            s = datetime.fromisoformat(start).date().isoformat()
        except ValueError:
            s = start
        return f"{s}/{_shift_back(end)}"
    return f"i/{_shift_back(v)}"


def norm_period_ferrox(p):
    if p is None:
        return "forever"
    if "instant" in p:
        return f"i/{p['instant']}"
    if "start" in p:
        return f"{p['start']}/{p['end']}"
    return "forever"


def norm_unit(v):
    if not v:
        return ""
    u = str(v).replace(" ", "")
    # xBRL-JSON omits the unit dimension when the unit is pure.
    return "" if u == "xbrli:pure" else u


def norm_dims(d):
    return tuple(sorted((k, str(v)) for k, v in d.items()))


def load_ferrox(path):
    data = json.load(open(path))
    facts = []
    for f in data["facts"]:
        dims = {k: v for k, v in (f.get("dims") or {}).items()}
        facts.append(
            {
                "concept": f["concept"],
                "entity": norm_entity(f.get("entity")),
                "period": norm_period_ferrox(f.get("period")),
                "unit": norm_unit(f.get("unit")),
                "dims": norm_dims(dims),
                "value": f.get("value"),
            }
        )
    return facts


def load_arelle(path):
    data = json.load(open(path))
    facts = []
    for _fid, f in data.get("facts", {}).items():
        d = f.get("dimensions", {})
        dims = {k: v for k, v in d.items() if k not in CORE_DIMS}
        facts.append(
            {
                "concept": d.get("concept", ""),
                "entity": norm_entity(d.get("entity")),
                "period": norm_period_oim(d.get("period")),
                "unit": norm_unit(d.get("unit")),
                "dims": norm_dims(dims),
                "value": f.get("value"),
            }
        )
    return facts


def fact_key(f):
    return (f["concept"], f["entity"], f["period"], f["unit"], f["dims"])


def enum_normalize(s):
    """Extensible-enum values: the raw doc writes 'http://ns#Local', OIM writes
    'pfx:Local'. Normalize whole tokens to their local part for comparison —
    only when every token matches one of those shapes."""
    toks = s.split()
    if not toks:
        return None
    out = []
    for t in toks:
        if "#" in t and t.split("#", 1)[0].startswith("http"):
            out.append(t.rsplit("#", 1)[-1])
        elif ":" in t and not t.startswith("http") and " " not in t:
            out.append(t.rsplit(":", 1)[-1])
        else:
            return None
    return " ".join(out)


def values_match(a, b):
    """Returns (matched, kind). kind 'arelle-null' = reference engine produced
    no value (its own transform gaps) — excluded from the headline rate but
    reported."""
    if a is None and b is None:
        return True, "nil"
    if b is None and a is not None:
        return False, "arelle-null"
    if a is None:
        return False, "nil-mismatch"
    sa, sb = str(a).strip(), str(b).strip()
    try:
        if Decimal(sa) == Decimal(sb):
            return True, "numeric"
        return False, "numeric-diff"
    except InvalidOperation:
        pass
    na, nb = " ".join(sa.split()), " ".join(sb.split())
    if na == nb:
        return True, "text"
    ea, eb = enum_normalize(na), enum_normalize(nb)
    if ea is not None and ea == eb:
        return True, "enum-normalized"
    # escaped text blocks: Arelle keeps raw HTML, ferroxbrl extracts text
    # (documented v0 behavior). Strip tags before comparing alnum content.
    import re
    def strip_tags(s):
        return re.sub(r"<[^>]*>", " ", s)
    ca = "".join(c for c in strip_tags(na) if c.isalnum())[:400]
    cb = "".join(c for c in strip_tags(nb) if c.isalnum())[:400]
    if ca and ca == cb:
        return True, "text-normalized"
    return False, "text-diff"


def main():
    ferrox_facts = load_ferrox(sys.argv[1])
    arelle_facts = load_arelle(sys.argv[2])

    fx = {}
    for f in ferrox_facts:
        fx.setdefault(fact_key(f), []).append(f)
    ar = {}
    for f in arelle_facts:
        ar.setdefault(fact_key(f), []).append(f)

    missing = [k for k in ar if k not in fx]
    extra = [k for k in fx if k not in ar]
    common = [k for k in ar if k in fx]

    match_counts = Counter()
    mismatches = []
    for k in common:
        a = ar[k][0]
        f = fx[k][0]
        ok, kind = values_match(f["value"], a["value"])
        match_counts[("match" if ok else "MISMATCH", kind)] += 1
        if not ok and kind != "arelle-null" and len(mismatches) < 15:
            mismatches.append((k[0], str(f["value"])[:80], str(a["value"])[:80], kind))

    n_ar, n_fx = len(arelle_facts), len(ferrox_facts)
    matched = sum(v for (s, _), v in match_counts.items() if s == "match")
    arelle_null = match_counts.get(("MISMATCH", "arelle-null"), 0)
    total_compared = sum(match_counts.values()) - arelle_null

    print(f"arelle facts:    {n_ar}")
    print(f"ferroxbrl facts: {n_fx}")
    print(f"key coverage:    {len(common)}/{len(ar)} arelle keys found by ferroxbrl "
          f"({100.0*len(common)/max(1,len(ar)):.2f}%)")
    print(f"  missing in ferroxbrl: {len(missing)}   extra in ferroxbrl: {len(extra)}")
    print(f"value match:     {matched}/{total_compared} "
          f"({100.0*matched/max(1,total_compared):.2f}%)"
          f"   [excludes {arelle_null} facts where Arelle itself emitted null]")
    for (status, kind), c in sorted(match_counts.items()):
        print(f"  {status:8} {kind:16} {c}")
    if missing[:10]:
        print("\nsample missing keys (in Arelle, not in ferroxbrl):")
        for k in missing[:10]:
            print(f"  {k[0]} period={k[2]} unit={k[3]} dims={len(k[4])}")
    if mismatches:
        print("\nsample value mismatches (concept, ferrox, arelle, kind):")
        for m in mismatches:
            print(f"  {m[0]}: {m[1]!r} vs {m[2]!r} [{m[3]}]")
    # exit code: 0 if headline >= 99% on both coverage and value match
    cov = len(common) / max(1, len(ar))
    val = matched / max(1, total_compared)
    sys.exit(0 if (cov >= 0.99 and val >= 0.99) else 1)


if __name__ == "__main__":
    main()
