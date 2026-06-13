//! SEC-specific (ixt-sec) lookup transforms.
//!
//! Implemented from the official EDGAR XBRL Guide §11.12 (April 2026,
//! sec.gov/files/edgar/filer-information/specifications/xbrl-guide.pdf) and
//! the EDGAR state & country code list
//! (sec.gov/submit-filings/filer-support-resources/edgar-state-country-codes).
//! Clean-room: no third-party processor code consulted.

use crate::transform::Transformed;

/// Normalize a lookup key: lowercase, collapse whitespace, strip punctuation.
fn norm_key(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Drop leading "the" and trailing legal-suffix tokens per the EDGAR guide
/// ("leading and trailing 'The', 'LLC', etc. ignored").
fn strip_legal(tokens: &[&str]) -> Vec<String> {
    const TRAILING: [&str; 5] = ["llc", "inc", "incorporated", "corp", "corporation"];
    let mut t: Vec<String> = tokens.iter().map(|s| s.to_string()).collect();
    while t.first().map(|s| s == "the").unwrap_or(false) {
        t.remove(0);
    }
    while t.last().map(|s| TRAILING.contains(&s.as_str())).unwrap_or(false) {
        t.pop();
    }
    t
}

// ---------------------------------------------------------------------------
// stateprovnameen — US state / Canadian province name -> postal abbreviation
// ---------------------------------------------------------------------------

const STATES: &[(&str, &str)] = &[
    ("alabama", "AL"), ("alaska", "AK"), ("arizona", "AZ"), ("arkansas", "AR"),
    ("california", "CA"), ("colorado", "CO"), ("connecticut", "CT"), ("delaware", "DE"),
    ("florida", "FL"), ("georgia", "GA"), ("hawaii", "HI"), ("idaho", "ID"),
    ("illinois", "IL"), ("indiana", "IN"), ("iowa", "IA"), ("kansas", "KS"),
    ("kentucky", "KY"), ("louisiana", "LA"), ("maine", "ME"), ("maryland", "MD"),
    ("massachusetts", "MA"), ("michigan", "MI"), ("minnesota", "MN"), ("mississippi", "MS"),
    ("missouri", "MO"), ("montana", "MT"), ("nebraska", "NE"), ("nevada", "NV"),
    ("new hampshire", "NH"), ("new jersey", "NJ"), ("new mexico", "NM"), ("new york", "NY"),
    ("north carolina", "NC"), ("north dakota", "ND"), ("ohio", "OH"), ("oklahoma", "OK"),
    ("oregon", "OR"), ("pennsylvania", "PA"), ("rhode island", "RI"), ("south carolina", "SC"),
    ("south dakota", "SD"), ("tennessee", "TN"), ("texas", "TX"), ("utah", "UT"),
    ("vermont", "VT"), ("virginia", "VA"), ("washington", "WA"), ("west virginia", "WV"),
    ("wisconsin", "WI"), ("wyoming", "WY"), ("district of columbia", "DC"),
    ("puerto rico", "PR"), ("guam", "GU"), ("virgin islands", "VI"),
    ("american samoa", "AS"), ("northern mariana islands", "MP"),
    // Canadian provinces/territories -> postal (EDGAR guide: "Ontario" -> "ON")
    ("alberta", "AB"), ("british columbia", "BC"), ("manitoba", "MB"),
    ("new brunswick", "NB"), ("newfoundland", "NL"), ("newfoundland and labrador", "NL"),
    ("nova scotia", "NS"), ("northwest territories", "NT"), ("nunavut", "NU"),
    ("ontario", "ON"), ("prince edward island", "PE"), ("quebec", "QC"),
    ("saskatchewan", "SK"), ("yukon", "YT"),
];

pub fn stateprovname(t: &str) -> Transformed {
    let k = norm_key(t);
    for (name, code) in STATES {
        if k == *name {
            return Transformed::Ok((*code).to_string());
        }
    }
    Transformed::Invalid(format!("unknown state/province {t:?}"))
}

// ---------------------------------------------------------------------------
// exchnameen — exchange legal name -> EDGAR exchange acronym (guide table)
// ---------------------------------------------------------------------------

const EXCHANGES: &[(&str, &str)] = &[
    ("new york stock exchange", "NYSE"),
    ("nasdaq global select market", "NASDAQ"),
    ("nasdaq global market", "NASDAQ"),
    ("nasdaq capital market", "NASDAQ"),
    ("nasdaq stock market", "NASDAQ"),
    ("nasdaq", "NASDAQ"),
    ("box exchange", "BOX"),
    ("nasdaq bx", "BX"),
    ("cboe c2 exchange", "C2"),
    ("cboe exchange", "CBOE"),
    ("chicago stock exchange", "CHX"),
    ("cboe byx exchange", "CboeBYX"),
    ("cboe bzx exchange", "CboeBZX"),
    ("cboe edga exchange", "CboeEDGA"),
    ("cboe edgx exchange", "CboeEDGX"),
    ("nasdaq gemx", "GEMX"),
    ("investors exchange", "IEX"),
    ("nasdaq ise", "ISE"),
    ("miami international securities exchange", "MIAX"),
    ("nasdaq mrx", "MRX"),
    ("nyse american", "NYSEAMER"),
    ("nyse arca", "NYSEArca"),
    ("nyse national", "NYSENAT"),
    ("miax pearl", "PEARL"),
    ("nasdaq phlx", "Phlx"),
];

pub fn exchname(t: &str) -> Transformed {
    let k = norm_key(t);
    let toks: Vec<&str> = k.split(' ').collect();
    let stripped = strip_legal(&toks).join(" ");
    for (name, code) in EXCHANGES {
        if stripped == *name {
            return Transformed::Ok((*code).to_string());
        }
    }
    Transformed::Invalid(format!("unknown exchange {t:?}"))
}

// ---------------------------------------------------------------------------
// entityfilercategoryen — normalize to the exact dei phrases
// ---------------------------------------------------------------------------

pub fn entityfilercategory(t: &str) -> Transformed {
    match norm_key(t).as_str() {
        "large accelerated filer" => Transformed::Ok("Large Accelerated Filer".to_string()),
        "accelerated filer" => Transformed::Ok("Accelerated Filer".to_string()),
        "non accelerated filer" => Transformed::Ok("Non-accelerated Filer".to_string()),
        _ => Transformed::Invalid(format!("unknown filer category {t:?}")),
    }
}

// ---------------------------------------------------------------------------
// countrynameen — country name -> ISO 3166-1 alpha-2 (guide: Canada->CA,
// Cayman Islands->KY, United Kingdom->GB)
// ---------------------------------------------------------------------------

const COUNTRIES_ISO: &[(&str, &str)] = &[
    ("united states", "US"), ("united states of america", "US"), ("usa", "US"),
    ("canada", "CA"), ("united kingdom", "GB"), ("uk", "GB"), ("great britain", "GB"),
    ("ireland", "IE"), ("switzerland", "CH"), ("cayman islands", "KY"), ("bermuda", "BM"),
    ("netherlands", "NL"), ("the netherlands", "NL"), ("luxembourg", "LU"), ("jersey", "JE"),
    ("guernsey", "GG"), ("israel", "IL"), ("japan", "JP"), ("germany", "DE"),
    ("france", "FR"), ("singapore", "SG"), ("china", "CN"), ("india", "IN"),
    ("australia", "AU"), ("brazil", "BR"), ("mexico", "MX"), ("spain", "ES"),
    ("italy", "IT"), ("sweden", "SE"), ("norway", "NO"), ("denmark", "DK"),
    ("finland", "FI"), ("belgium", "BE"), ("austria", "AT"), ("south korea", "KR"),
    ("republic of korea", "KR"), ("korea", "KR"), ("taiwan", "TW"), ("hong kong", "HK"),
    ("new zealand", "NZ"), ("portugal", "PT"), ("greece", "GR"), ("poland", "PL"),
    ("bahamas", "BS"), ("panama", "PA"), ("marshall islands", "MH"), ("liberia", "LR"),
    ("monaco", "MC"), ("cyprus", "CY"), ("malta", "MT"), ("isle of man", "IM"),
    ("gibraltar", "GI"), ("liechtenstein", "LI"), ("united arab emirates", "AE"),
    ("saudi arabia", "SA"), ("qatar", "QA"), ("turkey", "TR"), ("thailand", "TH"),
    ("malaysia", "MY"), ("indonesia", "ID"), ("philippines", "PH"), ("vietnam", "VN"),
    ("south africa", "ZA"), ("argentina", "AR"), ("chile", "CL"), ("colombia", "CO"),
    ("peru", "PE"), ("russia", "RU"), ("curacao", "CW"), ("iceland", "IS"),
    ("czech republic", "CZ"), ("hungary", "HU"), ("british virgin islands", "VG"),
];

pub fn countryname(t: &str) -> Transformed {
    let k = norm_key(t);
    for (name, code) in COUNTRIES_ISO {
        if k == *name {
            return Transformed::Ok((*code).to_string());
        }
    }
    Transformed::Invalid(format!("unknown country {t:?}"))
}

// ---------------------------------------------------------------------------
// edgarprovcountryen — state/province/country -> EDGAR code
// (US states keep their postal code; Canadian provinces A0-B0; countries from
// the EDGAR state & country code list)
// ---------------------------------------------------------------------------

const EDGAR_PROV_COUNTRY: &[(&str, &str)] = &[
    ("alberta", "A0"), ("british columbia", "A1"), ("manitoba", "A2"),
    ("new brunswick", "A3"), ("newfoundland", "A4"), ("nova scotia", "A5"),
    ("ontario", "A6"), ("prince edward island", "A7"), ("quebec", "A8"),
    ("saskatchewan", "A9"), ("yukon", "B0"), ("canada", "Z4"),
    ("australia", "C3"), ("austria", "C4"), ("belgium", "C9"), ("bermuda", "D0"),
    ("brazil", "D5"), ("cayman islands", "E9"), ("china", "F4"), ("denmark", "G7"),
    ("finland", "H9"), ("france", "I0"), ("germany", "2M"), ("guernsey", "Y7"),
    ("hong kong", "K3"), ("india", "K7"), ("ireland", "L2"), ("israel", "L3"),
    ("italy", "L6"), ("japan", "M0"), ("jersey", "Y9"), ("republic of korea", "M5"),
    ("south korea", "M5"), ("korea", "M5"), ("luxembourg", "N4"), ("mexico", "O5"),
    ("netherlands", "P7"), ("norway", "Q8"), ("singapore", "U0"), ("spain", "U3"),
    ("sweden", "V7"), ("switzerland", "V8"), ("taiwan", "F5"), ("united kingdom", "X0"),
];

pub fn edgarprovcountry(t: &str) -> Transformed {
    let k = norm_key(t);
    // US states map to their postal code in EDGAR.
    for (name, code) in STATES {
        if k == *name && code.len() == 2 && !matches!(*code, "AB" | "BC" | "MB" | "NB" | "NL" | "NS" | "NT" | "NU" | "ON" | "PE" | "QC" | "SK" | "YT") {
            return Transformed::Ok((*code).to_string());
        }
    }
    for (name, code) in EDGAR_PROV_COUNTRY {
        if k == *name {
            return Transformed::Ok((*code).to_string());
        }
    }
    Transformed::Invalid(format!("unknown EDGAR state/country {t:?}"))
}

// ---------------------------------------------------------------------------
// numwordsen — English number words -> integer ("nineteen hundred forty-four"
// -> 1944, "Seventy Thousand and one" -> 70001, "no"/"none" -> 0)
// ---------------------------------------------------------------------------

fn word_value(w: &str) -> Option<(u64, bool)> {
    // (value, is_multiplier)
    let v = match w {
        "zero" => 0, "one" => 1, "two" => 2, "three" => 3, "four" => 4, "five" => 5,
        "six" => 6, "seven" => 7, "eight" => 8, "nine" => 9, "ten" => 10,
        "eleven" => 11, "twelve" => 12, "thirteen" => 13, "fourteen" => 14,
        "fifteen" => 15, "sixteen" => 16, "seventeen" => 17, "eighteen" => 18,
        "nineteen" => 19, "twenty" => 20, "thirty" => 30, "forty" => 40,
        "fifty" => 50, "sixty" => 60, "seventy" => 70, "eighty" => 80, "ninety" => 90,
        "hundred" => return Some((100, true)),
        "thousand" => return Some((1_000, true)),
        "million" => return Some((1_000_000, true)),
        "billion" => return Some((1_000_000_000, true)),
        "trillion" => return Some((1_000_000_000_000, true)),
        _ => return None,
    };
    Some((v, false))
}

pub fn numwords(t: &str) -> Transformed {
    let k = norm_key(t);
    if k == "no" || k == "none" {
        return Transformed::Ok("0".to_string());
    }
    let mut total: u64 = 0;
    let mut current: u64 = 0;
    let mut any = false;
    for w in k.split(' ') {
        if w == "and" {
            continue;
        }
        match word_value(w) {
            Some((m, true)) => {
                if m == 100 {
                    current = if current == 0 { 100 } else { current * 100 };
                } else {
                    total += if current == 0 { m } else { current * m };
                    current = 0;
                }
                any = true;
            }
            Some((v, false)) => {
                current += v;
                any = true;
            }
            None => return Transformed::Invalid(format!("unknown number word {w:?} in {t:?}")),
        }
    }
    if !any {
        return Transformed::Invalid(format!("no number words in {t:?}"));
    }
    Transformed::Ok((total + current).to_string())
}

// ---------------------------------------------------------------------------
// durwordsen — "Five years, two months" -> P5Y2M; "10-year" -> P10Y;
// numbers may be digits or words
// ---------------------------------------------------------------------------

pub fn durwords(t: &str) -> Transformed {
    let k = norm_key(t); // hyphens already become spaces
    let mut y = 0u64;
    let mut mo = 0u64;
    let mut w = 0u64;
    let mut d = 0u64;
    let mut h = 0u64;
    let mut current: u64 = 0;
    let mut word_acc: Vec<&str> = Vec::new();
    let mut any_unit = false;
    let flush_words = |acc: &mut Vec<&str>, current: &mut u64| -> bool {
        if acc.is_empty() {
            return true;
        }
        match numwords(&acc.join(" ")) {
            Transformed::Ok(v) => {
                *current += v.parse::<u64>().unwrap_or(0);
                acc.clear();
                true
            }
            _ => false,
        }
    };
    for tok in k.split(' ') {
        if tok == "and" || tok == "a" {
            continue;
        }
        if let Ok(n) = tok.parse::<u64>() {
            current += n;
            continue;
        }
        let unit = match tok.trim_end_matches('s') {
            "year" | "yr" => Some(&mut y),
            "month" | "mo" => Some(&mut mo),
            "week" | "wk" => Some(&mut w),
            "day" => Some(&mut d),
            "hour" | "hr" => Some(&mut h),
            _ => None,
        };
        match unit {
            Some(slot) => {
                if !flush_words(&mut word_acc, &mut current) {
                    return Transformed::Invalid(format!("unparsable duration {t:?}"));
                }
                *slot += current;
                current = 0;
                any_unit = true;
            }
            None => word_acc.push(tok),
        }
    }
    if !any_unit {
        return Transformed::Invalid(format!("no duration unit in {t:?}"));
    }
    let mut out = String::from("P");
    if y > 0 {
        out.push_str(&format!("{y}Y"));
    }
    if mo > 0 {
        out.push_str(&format!("{mo}M"));
    }
    if w > 0 {
        out.push_str(&format!("{w}W"));
    }
    if d > 0 {
        out.push_str(&format!("{d}D"));
    }
    if h > 0 {
        out.push_str(&format!("T{h}H"));
    }
    if out == "P" {
        out.push_str("0D");
    }
    Transformed::Ok(out)
}

// ---------------------------------------------------------------------------
// dur{year,month,week,day,hour} — decimal counts -> truncated compound
// duration (guide: -22.3456 duryear -> -P22Y4M4D; 22.3456 durmonth -> P22M10D;
// 0 durweek -> P0D; 0.000001 durday -> P0D; 1000 durhour -> PT1000H)
// ---------------------------------------------------------------------------

pub fn dur_decimal(t: &str, unit: char) -> Transformed {
    let v = t.trim();
    let (neg, v) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v),
    };
    let val: f64 = match v.parse() {
        Ok(x) => x,
        Err(_) => return Transformed::Invalid(format!("unexpected duration content {t:?}")),
    };
    let whole = val.trunc() as u64;
    let frac = val.fract();
    let mut parts: Vec<String> = Vec::new();
    let mut time_part: Option<String> = None;
    match unit {
        'Y' => {
            if whole > 0 {
                parts.push(format!("{whole}Y"));
            }
            let months = frac * 12.0;
            let mw = months.trunc() as u64;
            if mw > 0 {
                parts.push(format!("{mw}M"));
            }
            let days = (months.fract() * 30.0).trunc() as u64;
            if days > 0 {
                parts.push(format!("{days}D"));
            }
        }
        'M' => {
            if whole > 0 {
                parts.push(format!("{whole}M"));
            }
            let days = (frac * 30.0).trunc() as u64;
            if days > 0 {
                parts.push(format!("{days}D"));
            }
        }
        'W' => {
            if whole > 0 {
                parts.push(format!("{whole}W"));
            }
            let days = (frac * 7.0).trunc() as u64;
            if days > 0 {
                parts.push(format!("{days}D"));
            }
        }
        'D' => {
            if whole > 0 {
                parts.push(format!("{whole}D"));
            }
        }
        'H' => {
            let mins = (frac * 60.0).trunc() as u64;
            let mut tp = String::new();
            if whole > 0 || mins == 0 {
                tp.push_str(&format!("{whole}H"));
            }
            if mins > 0 {
                tp.push_str(&format!("{mins}M"));
            }
            time_part = Some(tp);
        }
        _ => return Transformed::Invalid(format!("bad unit {unit}")),
    }
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push('P');
    if let Some(tp) = time_part {
        if parts.is_empty() && tp == "0H" {
            out.push_str("T0H");
        } else {
            out.push('T');
            out.push_str(&tp);
        }
    } else if parts.is_empty() {
        out.push_str("0D");
    } else {
        out.push_str(&parts.join(""));
    }
    // negative zero -> positive zero
    if out == "-P0D" {
        out = "P0D".to_string();
    }
    Transformed::Ok(out)
}

// ---------------------------------------------------------------------------
// datequarterend — "3rd quarter, 1999" -> 1999-09-30; "1999, Q4" -> 1999-12-31
// ---------------------------------------------------------------------------

pub fn datequarterend(t: &str) -> Transformed {
    let k = norm_key(t);
    let mut year: Option<i64> = None;
    let mut quarter: Option<u32> = None;
    for tok in k.split(' ') {
        if let Ok(n) = tok.parse::<i64>() {
            if (1000..=9999).contains(&n) {
                year = Some(n);
                continue;
            }
        }
        let q = match tok {
            "1st" | "first" | "q1" | "1" => Some(1),
            "2nd" | "second" | "q2" | "2" => Some(2),
            "3rd" | "third" | "q3" | "3" => Some(3),
            "4th" | "fourth" | "last" | "q4" | "4" => Some(4),
            _ => None,
        };
        if let Some(q) = q {
            quarter = Some(q);
        }
    }
    match (year, quarter) {
        (Some(y), Some(q)) => {
            let (m, d) = match q {
                1 => ("03", "31"),
                2 => ("06", "30"),
                3 => ("09", "30"),
                _ => ("12", "31"),
            };
            Transformed::Ok(format!("{y:04}-{m}-{d}"))
        }
        _ => Transformed::Invalid(format!("unparsable quarter-end {t:?}")),
    }
}

/// yesnoballotbox: checked -> "Yes", unchecked -> "No".
pub fn yesno_ballot_box(t: &str) -> Transformed {
    if t.contains('\u{2612}') || t.contains('\u{2611}') {
        Transformed::Ok("Yes".to_string())
    } else if t.contains('\u{2610}') || t.trim().is_empty() {
        Transformed::Ok("No".to_string())
    } else {
        Transformed::Invalid(format!("unexpected ballot box content {t:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Transformed;

    fn ok(t: Transformed) -> String {
        match t {
            Transformed::Ok(s) => s,
            Transformed::Unknown => panic!("unknown"),
            Transformed::Invalid(e) => panic!("invalid: {e}"),
        }
    }

    #[test]
    fn states_and_countries() {
        assert_eq!(ok(stateprovname("California")), "CA");
        assert_eq!(ok(stateprovname("Ontario")), "ON");
        assert_eq!(ok(countryname("Canada")), "CA");
        assert_eq!(ok(countryname("Cayman Islands")), "KY");
        assert_eq!(ok(countryname("United Kingdom")), "GB");
        assert_eq!(ok(countryname("USA")), "US");
        assert_eq!(ok(edgarprovcountry("Ontario")), "A6");
        assert_eq!(ok(edgarprovcountry("Canada")), "Z4");
        assert_eq!(ok(edgarprovcountry("Cayman Islands")), "E9");
        assert_eq!(ok(edgarprovcountry("United Kingdom")), "X0");
        assert_eq!(ok(edgarprovcountry("Delaware")), "DE");
        assert_eq!(ok(edgarprovcountry("Ireland")), "L2");
        assert_eq!(ok(edgarprovcountry("Switzerland")), "V8");
    }

    #[test]
    fn exchanges() {
        assert_eq!(ok(exchname("The New York Stock Exchange")), "NYSE");
        assert_eq!(ok(exchname("New York Stock Exchange LLC")), "NYSE");
        assert_eq!(ok(exchname("NASDAQ Global Select Market")), "NASDAQ");
        assert_eq!(ok(exchname("The Nasdaq Stock Market LLC")), "NASDAQ");
        assert_eq!(ok(exchname("NYSE American LLC")), "NYSEAMER");
        assert_eq!(ok(exchname("NYSE Arca, Inc.")), "NYSEArca");
        assert_eq!(ok(exchname("Cboe BZX Exchange, Inc.")), "CboeBZX");
    }

    #[test]
    fn filer_category() {
        assert_eq!(ok(entityfilercategory("Large accelerated filer")), "Large Accelerated Filer");
        assert_eq!(ok(entityfilercategory("Non-Accelerated Filer")), "Non-accelerated Filer");
        assert_eq!(ok(entityfilercategory("accelerated filer ")), "Accelerated Filer");
    }

    #[test]
    fn num_words() {
        assert_eq!(ok(numwords("nineteen hundred forty-four")), "1944");
        assert_eq!(ok(numwords("Seventy Thousand and one")), "70001");
        assert_eq!(ok(numwords("no")), "0");
        assert_eq!(ok(numwords("None")), "0");
        assert_eq!(ok(numwords("Eighty-eight")), "88");
        assert_eq!(ok(numwords("Five")), "5");
    }

    #[test]
    fn dur_words() {
        assert_eq!(ok(durwords("Five years, two months")), "P5Y2M");
        assert_eq!(ok(durwords("9 years, 2 months")), "P9Y2M");
        assert_eq!(ok(durwords("10 years")), "P10Y");
        assert_eq!(ok(durwords("10-year")), "P10Y");
        assert_eq!(ok(durwords("12-month")), "P12M");
        assert_eq!(ok(durwords("15 days")), "P15D");
    }

    #[test]
    fn dur_decimals_per_guide() {
        assert_eq!(ok(dur_decimal("-22.3456", 'Y')), "-P22Y4M4D");
        assert_eq!(ok(dur_decimal("22.3456", 'M')), "P22M10D");
        assert_eq!(ok(dur_decimal("0", 'W')), "P0D");
        assert_eq!(ok(dur_decimal("0.000001", 'D')), "P0D");
        assert_eq!(ok(dur_decimal("1000", 'H')), "PT1000H");
        assert_eq!(ok(dur_decimal("5", 'Y')), "P5Y");
    }

    #[test]
    fn quarter_end() {
        assert_eq!(ok(datequarterend("3rd quarter, 1999")), "1999-09-30");
        assert_eq!(ok(datequarterend("Last quarter 1999")), "1999-12-31");
        assert_eq!(ok(datequarterend("1999, Q4")), "1999-12-31");
        assert_eq!(ok(datequarterend("1999 1st Quarter")), "1999-03-31");
    }
}
