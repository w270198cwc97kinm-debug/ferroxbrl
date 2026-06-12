//! ferroxbrl-core — fast inline XBRL (iXBRL) fact extraction.
//!
//! Clean-room implementation against the public XBRL 2.1 / iXBRL 1.1 / OIM
//! specifications and conformance suites. No AGPL code consulted.

pub mod model;
pub mod parser;
pub mod transform;

pub use model::{Context, Document, Fact, FactKind, Period, Unit};
pub use parser::Error;

/// Extract facts, contexts and units from an inline XBRL document string.
pub fn extract_str(xml: &str) -> Result<Document, Error> {
    parser::Parser::new().parse(xml)
}

/// Extract from a file path (reads as UTF-8 with lossy fallback).
pub fn extract_path(path: &std::path::Path) -> Result<Document, Error> {
    let bytes = std::fs::read(path).map_err(|e| Error(format!("read {}: {e}", path.display())))?;
    let xml = String::from_utf8_lossy(&bytes);
    extract_str(&xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"
      xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ixt="http://www.xbrl.org/inlineXBRL/transformation/2015-02-26"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xmlns:us-gaap="http://fasb.org/us-gaap/2024"
      xmlns:dei="http://xbrl.sec.gov/dei/2024"
      xmlns:srt="http://fasb.org/srt/2024">
<body>
 <div style="display:none">
  <ix:header>
   <ix:hidden>
    <dei:EntityCentralIndexKey contextRef="c1">0000320193</dei:EntityCentralIndexKey>
   </ix:hidden>
   <ix:resources>
    <xbrli:context id="c1">
     <xbrli:entity>
      <xbrli:identifier scheme="http://www.sec.gov/CIK">0000320193</xbrli:identifier>
     </xbrli:entity>
     <xbrli:period>
      <xbrli:startDate>2024-01-01</xbrli:startDate>
      <xbrli:endDate>2024-12-31</xbrli:endDate>
     </xbrli:period>
    </xbrli:context>
    <xbrli:context id="c2">
     <xbrli:entity>
      <xbrli:identifier scheme="http://www.sec.gov/CIK">0000320193</xbrli:identifier>
     </xbrli:entity>
     <xbrli:period>
      <xbrli:instant>2024-12-31</xbrli:instant>
     </xbrli:period>
     <xbrli:scenario>
      <xbrldi:explicitMember dimension="srt:ProductOrServiceAxis">us-gaap:ProductMember</xbrldi:explicitMember>
     </xbrli:scenario>
    </xbrli:context>
    <xbrli:unit id="usd">
     <xbrli:measure>iso4217:USD</xbrli:measure>
    </xbrli:unit>
    <xbrli:unit id="usdPerShare">
     <xbrli:divide>
      <xbrli:unitNumerator><xbrli:measure>iso4217:USD</xbrli:measure></xbrli:unitNumerator>
      <xbrli:unitDenominator><xbrli:measure>xbrli:shares</xbrli:measure></xbrli:unitDenominator>
     </xbrli:divide>
    </xbrli:unit>
   </ix:resources>
  </ix:header>
 </div>
 <p>Revenue was $<ix:nonFraction name="us-gaap:Revenues" contextRef="c1" unitRef="usd"
     scale="6" decimals="-6" format="ixt:num-dot-decimal">391,035</ix:nonFraction> million.</p>
 <p>Loss of <ix:nonFraction name="us-gaap:NetIncomeLoss" contextRef="c1" unitRef="usd"
     scale="3" sign="-" format="ixt:num-dot-decimal">1,234.5</ix:nonFraction> thousand.</p>
 <p>EPS <ix:nonFraction name="us-gaap:EarningsPerShareBasic" contextRef="c1" unitRef="usdPerShare"
     decimals="2" format="ixt:num-dot-decimal">6.08</ix:nonFraction></p>
 <p>Segment assets
  <ix:nonFraction name="us-gaap:Assets" contextRef="c2" unitRef="usd" scale="6"
      format="ixt:num-dot-decimal">12<ix:exclude><span>(see note 4)</span></ix:exclude>,500</ix:nonFraction>
 </p>
 <p>Date: <ix:nonNumeric name="dei:DocumentPeriodEndDate" contextRef="c1"
     format="ixt:datemonthdayyearen">December 31, 2024</ix:nonNumeric></p>
 <p>Description starts <ix:nonNumeric name="us-gaap:NatureOfOperations" contextRef="c1"
     continuedAt="cont1">part one</ix:nonNumeric></p>
 <div><ix:continuation id="cont1"> and part two</ix:continuation></div>
 <p>Nil: <ix:nonFraction name="us-gaap:Liabilities" contextRef="c2" unitRef="usd" xsi:nil="true"/></p>
</body>
</html>"#;

    #[test]
    fn parses_minimal_document() {
        let doc = extract_str(MINIMAL).expect("parse");
        assert_eq!(doc.contexts.len(), 2, "contexts: {:?}", doc.contexts.keys());
        assert_eq!(doc.units.len(), 2);
        // hidden + 5 nonFraction + 2 nonNumeric = 8 facts
        assert_eq!(doc.facts.len(), 8, "facts: {:#?}", doc.facts);

        let get = |name: &str| {
            doc.facts
                .iter()
                .find(|f| f.concept == name)
                .unwrap_or_else(|| panic!("fact {name} missing"))
        };

        assert_eq!(get("us-gaap:Revenues").value.as_deref(), Some("391035000000"));
        assert_eq!(get("us-gaap:NetIncomeLoss").value.as_deref(), Some("-1234500"));
        assert_eq!(get("us-gaap:EarningsPerShareBasic").value.as_deref(), Some("6.08"));
        // exclude content dropped: 12,500 * 10^6
        assert_eq!(get("us-gaap:Assets").value.as_deref(), Some("12500000000"));
        assert_eq!(
            get("dei:DocumentPeriodEndDate").value.as_deref(),
            Some("2024-12-31")
        );
        assert_eq!(
            get("us-gaap:NatureOfOperations").value.as_deref(),
            Some("part one and part two")
        );
        assert_eq!(get("us-gaap:Liabilities").value, None);
        assert_eq!(
            get("dei:EntityCentralIndexKey").value.as_deref(),
            Some("0000320193")
        );

        // context joins
        let c2 = &doc.contexts["c2"];
        assert_eq!(c2.entity, "0000320193");
        assert_eq!(
            c2.dims.get("srt:ProductOrServiceAxis").map(|s| s.as_str()),
            Some("us-gaap:ProductMember")
        );
        let usd_per_share = &doc.units["usdPerShare"];
        assert_eq!(usd_per_share.display(), "iso4217:USD/xbrli:shares");
    }
}
