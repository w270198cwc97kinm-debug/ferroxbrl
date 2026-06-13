# ferroxbrl

**Fast inline XBRL (iXBRL) fact extraction in Rust.** Parses SEC EDGAR / ESEF
inline XBRL filings and emits facts (with contexts, units, dimensions) as JSON
— in milliseconds, with no taxonomy server round-trips.

> Status: **v0.0.x — working vertical slice.** Validated against Arelle on real
> EDGAR filings (see below). Not yet a validator; extraction only.

## Why

Arelle (the reference Python processor, ~250k downloads/month) is the engine
behind most XBRL pipelines, and bulk fact extraction through it is slow.
ferroxbrl is a clean-room Rust implementation of the extraction path:
zero-copy XML streaming, single pass, no DTS download required for fact
extraction.

- MIT/Apache-2.0 (permissive — usable inside commercial data pipelines)
- Single binary; Python wheels planned (pyo3/maturin)
- Honest benchmarking: we publish both what we measure *and what we skip*

## Quick start

CLI:
```sh
cargo build --release
./target/release/ferroxbrl extract filing.htm --pretty -o facts.json
```

Python (built with [maturin](https://www.maturin.rs/)):
```sh
maturin develop --release        # or: pip install <built wheel>
```
```python
import ferroxbrl
doc = ferroxbrl.extract("filing.htm")     # path or raw iXBRL string
doc["fact_count"]                          # -> 1127
doc["facts"][0]                            # {'concept': 'us-gaap:Revenues', 'value': '391035000000', ...}
```

## Correctness & speed

### Single filing (Apple FY2024 10-K, 1.5 MB iXBRL)
Reproduce: `testdata/fetch.sh && bench/run.sh testdata/aapl-20240928.htm`

| metric | result |
|---|---|
| facts extracted | 1127 (identical count to Arelle) |
| fact-key coverage vs Arelle OIM | **100.00 %** (1043/1043 deduplicated keys) |
| value match vs Arelle OIM | **100.00 %** (1014/1014 compared) |
| ferroxbrl parse time | ~12 ms (in-process), ~0.4 s end-to-end CLI |
| Arelle, warm DTS cache | ~1.1 s (same machine, extraction-only, no `--validate`) |

### 100 largest US filers (latest 10-Q each, ~131k facts)
Reproduce: `bench/run100.py` (writes `bench/results-100.json` / `.md`)

| metric | result |
|---|---|
| aggregate key coverage vs Arelle | **99.966 %** (131,292/131,336) |
| aggregate value match vs Arelle | **99.875 %** (128,253/128,413) |
| median per-filing speedup | **49.1×** (p10 33.7× / p90 71.4×) |
| total wall time | ferroxbrl 3.3 s vs Arelle 149 s |

The residual ~0.13 % are escaped rich-text `TextBlock` facts (HTML tables) where
markup is dropped — a documented v0 limitation; the comparison normalizes both
sides. 2,879 facts where **Arelle itself** emitted null (its own `ixt-sec`
transform gaps — ferroxbrl produces correct values, independently verified
against the SEC submissions API: incorporation state, exchange acronym and
filer category all match) are excluded from the rate and reported.

**Honest-bench caveats** (also printed by the harness): Arelle resolves the
full DTS and is concept-aware; ferroxbrl parses the instance document only —
which is why it is fast and why it cannot (yet) validate. One run is not a
proof of universal correctness; the harness is there so you can check your own
filings.

**Honest-bench caveats** (also printed by the harness):
- Arelle resolves the full DTS (taxonomy schemas) and is concept-aware;
  ferroxbrl parses the instance document only. That is precisely why it is
  fast — and why it cannot (yet) validate.
- `escape="true"` text blocks are captured as text content; markup is dropped
  (comparison normalizes both sides before matching).
- One filing is not a benchmark suite. The 100-filing EDGAR/ESEF run is the
  Day-14 gate for this project (see kill criteria below).

## What works today

- iXBRL 1.0/1.1: `nonFraction`, `nonNumeric`, `hidden` facts, `continuation`
  chains, `exclude`, `sign`/`scale`/`decimals`, `xsi:nil`
- Contexts: instant/duration/forever, entity, explicit dimensions, typed
  dimensions (text value)
- Units incl. divide (e.g. `iso4217:USD/xbrli:shares`)
- TRR transforms: numeric (dot/comma-decimal, fixed-zero…), dates (English
  month-name v1–v4 names, numeric, `gMonthDay`/`gYearMonth`)
- SEC `ixt-sec` lookups: `stateprovnameen`, `countrynameen`,
  `edgarprovcountryen`, `exchnameen`, `entityfilercategoryen`, `numwordsen`,
  `durwordsen`, `dur{year,month,week,day,hour}` (decimal compound form),
  `datequarterend`, `bool`/`yesnoballotbox` — tables from the EDGAR XBRL Guide
- Unknown transforms degrade gracefully and are reported as warnings
- Python bindings (pyo3): `ferroxbrl.extract()`

## Not yet (tracked, in order)

- Multi-document inline XBRL document sets (IXDS)
- Markup-preserving escaped text blocks; `ix:tuple`/`ix:fraction`/footnotes
- XSD/calculation validation (different product layer)
- Published wheels (PyPI), Parquet output

## Design notes

Clean-room implementation against the public XBRL 2.1 / iXBRL 1.1 / OIM specs
and conformance materials. No AGPL code was consulted.

## License

MIT OR Apache-2.0
