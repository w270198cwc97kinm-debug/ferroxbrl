"""Smoke test for the ferroxbrl Python extension.

Run after `maturin develop`:  python -m pytest python/tests
"""
import ferroxbrl

MINIMAL = """<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"
      xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ixt="http://www.xbrl.org/inlineXBRL/transformation/2015-02-26"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:us-gaap="http://fasb.org/us-gaap/2024">
<body>
 <div style="display:none"><ix:header><ix:resources>
   <xbrli:context id="c1"><xbrli:entity>
     <xbrli:identifier scheme="http://www.sec.gov/CIK">0000320193</xbrli:identifier>
   </xbrli:entity><xbrli:period>
     <xbrli:startDate>2024-01-01</xbrli:startDate><xbrli:endDate>2024-12-31</xbrli:endDate>
   </xbrli:period></xbrli:context>
   <xbrli:unit id="usd"><xbrli:measure>iso4217:USD</xbrli:measure></xbrli:unit>
 </ix:resources></ix:header></div>
 <p><ix:nonFraction name="us-gaap:Revenues" contextRef="c1" unitRef="usd"
    scale="6" format="ixt:num-dot-decimal">391,035</ix:nonFraction></p>
</body></html>"""


def test_version():
    assert isinstance(ferroxbrl.__version__, str)


def test_extract_from_string():
    doc = ferroxbrl.extract(MINIMAL)
    assert doc["fact_count"] == 1
    assert doc["context_count"] == 1
    assert doc["unit_count"] == 1
    f = doc["facts"][0]
    assert f["concept"] == "us-gaap:Revenues"
    assert f["value"] == "391035000000"
    assert f["unit"] == "iso4217:USD"
    assert f["entity"] == "0000320193"


def test_extract_facts_helper():
    facts = ferroxbrl.extract_facts(MINIMAL)
    assert isinstance(facts, list)
    assert facts[0]["concept"] == "us-gaap:Revenues"


def test_missing_file_raises():
    import pytest
    with pytest.raises((FileNotFoundError, ValueError)):
        ferroxbrl.extract("/no/such/file.htm")
