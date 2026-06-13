//! Transformation Registry (TRR) value transforms.
//!
//! Implements the numeric transforms that dominate EDGAR/ESEF filings plus the
//! most common date/boolean ones. Unknown transforms fall back to a
//! best-effort numeric cleanup (for nonFraction) or the raw value
//! (for nonNumeric), and are surfaced as warnings so the Arelle-diff harness
//! can drive coverage in later weeks.

/// Result of applying a transform.
pub enum Transformed {
    Ok(String),
    /// Transform not implemented — caller decides fallback and warns.
    Unknown,
    /// Content did not match the transform's expected lexical space.
    Invalid(String),
}

/// Normalize a transform QName to a comparable key: local name, lowercased,
/// hyphens removed ("ixt4:num-dot-decimal" -> "numdotdecimal").
pub fn transform_key(format: &str) -> String {
    let local = format.rsplit(':').next().unwrap_or(format);
    local.chars().filter(|c| *c != '-').collect::<String>().to_ascii_lowercase()
}

/// Apply a TRR transform to raw element text.
pub fn apply(format: &str, raw: &str) -> Transformed {
    let key = transform_key(format);
    let t = collapse_ws(raw);
    match key.as_str() {
        // --- fixed outputs -------------------------------------------------
        "fixedzero" | "zerodash" | "numdash" => Transformed::Ok("0".to_string()),
        "fixedempty" | "nocontent" => Transformed::Ok(String::new()),
        "fixedtrue" | "booleantrue" => Transformed::Ok("true".to_string()),
        "fixedfalse" | "booleanfalse" => Transformed::Ok("false".to_string()),
        // --- numeric -------------------------------------------------------
        "numdotdecimal" | "numdotdecimalapos" => num_grouped(&t, &[',', ' ', '\u{a0}', '\u{2009}', '\u{202f}', '\''], '.'),
        "numcommadecimal" | "numcommadecimalapos" => num_grouped(&t, &['.', ' ', '\u{a0}', '\u{2009}', '\u{202f}', '\''], ','),
        "numunitdecimal" | "numunitdecimalapos" => Transformed::Unknown,
        // --- dates (English month names, EDGAR-common) ---------------------
        // TRR v1/v2 names ("datemonthdayyearen") and v3+ names
        // ("date-monthname-day-year-en") normalize to different keys; both map
        // to the same parsers.
        "datemonthdayyearen" | "datemonthnamedayyearen" => date_mdy_en(&t),
        "datedaymonthyearen" | "datedaymonthnameyearen" => date_dmy_en(&t),
        "datemonthdayyear" => date_mdy_numeric(&t),
        "datedaymonthyear" => date_dmy_numeric(&t),
        "dateyearmonthday" => date_ymd_numeric(&t),
        "datemonthyearen" | "datemonthnameyearen" => date_my_en(&t),
        "datemonthdayen" | "datemonthnamedayen" => date_md_en(&t),
        "datemonthday" => date_md_numeric(&t),
        "dateyearmonth" => date_ym_numeric(&t),
        // --- SEC-specific (ixt-sec) -----------------------------------------
        "boolballotbox" => bool_ballot_box(&t),
        "yesnoballotbox" => crate::sec_lookup::yesno_ballot_box(&t),
        "duryear" => crate::sec_lookup::dur_decimal(&t, 'Y'),
        "durmonth" => crate::sec_lookup::dur_decimal(&t, 'M'),
        "durweek" => crate::sec_lookup::dur_decimal(&t, 'W'),
        "durday" => crate::sec_lookup::dur_decimal(&t, 'D'),
        "durhour" => crate::sec_lookup::dur_decimal(&t, 'H'),
        "durwordsen" => crate::sec_lookup::durwords(&t),
        "numwordsen" => crate::sec_lookup::numwords(&t),
        "datequarterend" => crate::sec_lookup::datequarterend(&t),
        "stateprovnameen" => crate::sec_lookup::stateprovname(&t),
        "countrynameen" => crate::sec_lookup::countryname(&t),
        "edgarprovcountryen" => crate::sec_lookup::edgarprovcountry(&t),
        "exchnameen" => crate::sec_lookup::exchname(&t),
        "entityfilercategoryen" => crate::sec_lookup::entityfilercategory(&t),
        _ => Transformed::Unknown,
    }
}

/// Collapse all unicode whitespace runs to single spaces and trim.
pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip grouping separators, normalize the decimal separator to '.'.
fn num_grouped(t: &str, group: &[char], decimal: char) -> Transformed {
    let mut out = String::with_capacity(t.len());
    let mut seen_decimal = false;
    for c in t.chars() {
        if group.contains(&c) {
            continue;
        }
        if c == decimal {
            if seen_decimal {
                return Transformed::Invalid(format!("multiple decimal separators in {t:?}"));
            }
            seen_decimal = true;
            out.push('.');
        } else if c.is_ascii_digit() {
            out.push(c);
        } else {
            return Transformed::Invalid(format!("unexpected char {c:?} in numeric content {t:?}"));
        }
    }
    if out.is_empty() || out == "." {
        return Transformed::Invalid(format!("empty numeric content {t:?}"));
    }
    Transformed::Ok(out)
}

fn month_en(name: &str) -> Option<u32> {
    let n = name.trim_end_matches('.').to_ascii_lowercase();
    const MONTHS: [&str; 12] = [
        "january", "february", "march", "april", "may", "june", "july", "august", "september",
        "october", "november", "december",
    ];
    MONTHS
        .iter()
        .position(|m| *m == n || (n.len() >= 3 && m.starts_with(&n)))
        .map(|i| i as u32 + 1)
}

fn fmt_date(y: &str, m: u32, d: u32) -> Transformed {
    let year: i32 = match y.parse() {
        Ok(v) => v,
        Err(_) => return Transformed::Invalid(format!("bad year {y:?}")),
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Transformed::Invalid(format!("bad date {y}-{m}-{d}"));
    }
    Transformed::Ok(format!("{year:04}-{m:02}-{d:02}"))
}

/// "December 31, 2024" / "Dec. 31, 2024"
fn date_mdy_en(t: &str) -> Transformed {
    let cleaned = t.replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() != 3 {
        return Transformed::Invalid(format!("expected 'Month D, Y', got {t:?}"));
    }
    match (month_en(parts[0]), parts[1].parse::<u32>()) {
        (Some(m), Ok(d)) => fmt_date(parts[2], m, d),
        _ => Transformed::Invalid(format!("unparsable en date {t:?}")),
    }
}

/// "31 December 2024"
fn date_dmy_en(t: &str) -> Transformed {
    let cleaned = t.replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() != 3 {
        return Transformed::Invalid(format!("expected 'D Month Y', got {t:?}"));
    }
    match (parts[0].parse::<u32>(), month_en(parts[1])) {
        (Ok(d), Some(m)) => fmt_date(parts[2], m, d),
        _ => Transformed::Invalid(format!("unparsable en date {t:?}")),
    }
}

/// "December 31" -> "--12-31" (xs:gMonthDay)
fn date_md_en(t: &str) -> Transformed {
    let cleaned = t.replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() != 2 {
        return Transformed::Invalid(format!("expected 'Month D', got {t:?}"));
    }
    match (month_en(parts[0]), parts[1].parse::<u32>()) {
        (Some(m), Ok(d)) if (1..=31).contains(&d) => Transformed::Ok(format!("--{m:02}-{d:02}")),
        _ => Transformed::Invalid(format!("unparsable en month-day {t:?}")),
    }
}

/// SEC ballot box: checked box -> true, unchecked -> false.
fn bool_ballot_box(t: &str) -> Transformed {
    if t.contains('\u{2612}') || t.contains('\u{2611}') {
        Transformed::Ok("true".to_string())
    } else if t.contains('\u{2610}') || t.is_empty() {
        Transformed::Ok("false".to_string())
    } else {
        Transformed::Invalid(format!("unexpected ballot box content {t:?}"))
    }
}

/// "December 2024" -> "2024-12"
fn date_my_en(t: &str) -> Transformed {
    let cleaned = t.replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() != 2 {
        return Transformed::Invalid(format!("expected 'Month Y', got {t:?}"));
    }
    match (month_en(parts[0]), parts[1].parse::<i32>()) {
        (Some(m), Ok(y)) => Transformed::Ok(format!("{y:04}-{m:02}")),
        _ => Transformed::Invalid(format!("unparsable en month-year {t:?}")),
    }
}

fn split_numeric_date(t: &str) -> Option<Vec<u32>> {
    let parts: Vec<&str> = t
        .split(|c: char| c == '/' || c == '-' || c == '.' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    parts.iter().map(|p| p.parse::<u32>().ok()).collect()
}

fn date_mdy_numeric(t: &str) -> Transformed {
    match split_numeric_date(t) {
        Some(v) => fmt_date(&v[2].to_string(), v[0], v[1]),
        None => Transformed::Invalid(format!("unparsable numeric date {t:?}")),
    }
}

fn date_dmy_numeric(t: &str) -> Transformed {
    match split_numeric_date(t) {
        Some(v) => fmt_date(&v[2].to_string(), v[1], v[0]),
        None => Transformed::Invalid(format!("unparsable numeric date {t:?}")),
    }
}

fn split_numeric_pair(t: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = t
        .split(|c: char| c == '/' || c == '-' || c == '.' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 2 {
        return None;
    }
    match (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
        (Ok(a), Ok(b)) => Some((a, b)),
        _ => None,
    }
}

/// "12-31" / "12/31" -> "--12-31" (xs:gMonthDay)
fn date_md_numeric(t: &str) -> Transformed {
    match split_numeric_pair(t) {
        Some((m, d)) if (1..=12).contains(&m) && (1..=31).contains(&d) => {
            Transformed::Ok(format!("--{m:02}-{d:02}"))
        }
        _ => Transformed::Invalid(format!("unparsable numeric month-day {t:?}")),
    }
}

/// "2024-12" / "2024/12" -> "2024-12" (xs:gYearMonth)
fn date_ym_numeric(t: &str) -> Transformed {
    match split_numeric_pair(t) {
        Some((y, m)) if y >= 1000 && (1..=12).contains(&m) => {
            Transformed::Ok(format!("{y:04}-{m:02}"))
        }
        _ => Transformed::Invalid(format!("unparsable numeric year-month {t:?}")),
    }
}

fn date_ymd_numeric(t: &str) -> Transformed {
    match split_numeric_date(t) {
        Some(v) => fmt_date(&v[0].to_string(), v[1], v[2]),
        None => Transformed::Invalid(format!("unparsable numeric date {t:?}")),
    }
}

/// Best-effort numeric cleanup when no/unknown format on a nonFraction.
pub fn fallback_numeric(raw: &str) -> Transformed {
    num_grouped(&collapse_ws(raw), &[',', ' ', '\u{a0}', '\u{2009}', '\u{202f}'], '.')
}

/// Shift the decimal point of a canonical decimal string by 10^n (the iXBRL
/// scale attribute), using exact string arithmetic — no floats.
pub fn shift_decimal(value: &str, n: i32) -> String {
    if n == 0 {
        return value.to_string();
    }
    let (neg, digits) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let (int_part, frac_part) = match digits.split_once('.') {
        Some((i, f)) => (i.to_string(), f.to_string()),
        None => (digits.to_string(), String::new()),
    };
    // Work with a pure digit string and a point position from the left.
    let mut ds: Vec<u8> = format!("{int_part}{frac_part}").into_bytes();
    let mut point = int_part.len() as i32 + n;
    // Pad as needed.
    while point > ds.len() as i32 {
        ds.push(b'0');
    }
    while point < 0 {
        ds.insert(0, b'0');
        point += 1;
    }
    let point = point as usize;
    let int_s: String = String::from_utf8(ds[..point].to_vec()).unwrap();
    let frac_s: String = String::from_utf8(ds[point..].to_vec()).unwrap();
    let int_trim = int_s.trim_start_matches('0');
    let int_final = if int_trim.is_empty() { "0" } else { int_trim };
    let frac_trim = frac_s.trim_end_matches('0');
    let mut out = String::new();
    if neg && !(int_final == "0" && frac_trim.is_empty()) {
        out.push('-');
    }
    out.push_str(int_final);
    if !frac_trim.is_empty() {
        out.push('.');
        out.push_str(frac_trim);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(t: Transformed) -> String {
        match t {
            Transformed::Ok(s) => s,
            Transformed::Unknown => panic!("unknown"),
            Transformed::Invalid(e) => panic!("invalid: {e}"),
        }
    }

    #[test]
    fn numeric_transforms() {
        assert_eq!(ok(apply("ixt:num-dot-decimal", " 1,234,567.89 ")), "1234567.89");
        assert_eq!(ok(apply("ixt4:num-comma-decimal", "1.234.567,89")), "1234567,89".replace(',', "."));
        assert_eq!(ok(apply("ixt:fixed-zero", "—")), "0");
    }

    #[test]
    fn date_transforms() {
        assert_eq!(ok(apply("ixt:datemonthdayyearen", "December 31, 2024")), "2024-12-31");
        assert_eq!(ok(apply("ixt:datemonthdayyearen", "Mar. 5, 2025")), "2025-03-05");
        assert_eq!(ok(apply("ixt:datedaymonthyearen", "31 December 2024")), "2024-12-31");
        // TRR v3+ hyphenated names
        assert_eq!(ok(apply("ixt:date-monthname-day-year-en", "September 28, 2024")), "2024-09-28");
        assert_eq!(ok(apply("ixt:date-monthname-day-en", "September 28")), "--09-28");
    }

    #[test]
    fn sec_transforms() {
        assert_eq!(ok(apply("ixt-sec:boolballotbox", "\u{2612}")), "true");
        assert_eq!(ok(apply("ixt-sec:boolballotbox", "\u{2610}")), "false");
        assert_eq!(ok(apply("ixt-sec:duryear", "5")), "P5Y");
        assert_eq!(ok(apply("ixt-sec:durmonth", "6")), "P6M");
    }

    #[test]
    fn decimal_shift() {
        assert_eq!(shift_decimal("1234.5", 3), "1234500");
        assert_eq!(shift_decimal("1234.5", -2), "12.345");
        assert_eq!(shift_decimal("5", 6), "5000000");
        assert_eq!(shift_decimal("-0.5", 2), "-50");
        assert_eq!(shift_decimal("0", 6), "0");
    }
}
