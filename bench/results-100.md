# ferroxbrl — 100-filing EDGAR parity run

- filings compared OK: **100** (failures/skips: 32)
- aggregate key coverage: **99.966%** (131292/131336)
- aggregate value match: **99.868%** (128243/128413)
- median per-filing speedup (CLI vs warm Arelle): **49.1x**
- total wall: ferroxbrl 3.3s vs Arelle 149.2s

## Worst filings by value match

| ticker | form | facts | coverage | value match | top mismatch kinds |
|---|---|---|---|---|---|
| GE | 10-Q | 1299 | 100.00% | 98.91% | arelle-null:28, text-diff:13 |
| PG | 10-Q | 1028 | 99.89% | 99.04% | text-diff:9, arelle-null:7 |
| BA | 10-Q | 1154 | 100.00% | 99.22% | arelle-null:30, text-diff:8 |
| RTX | 10-Q | 1103 | 100.00% | 99.36% | arelle-null:26, text-diff:6 |
| JNJ | 10-Q | 1473 | 100.00% | 99.50% | arelle-null:25, text-diff:7 |
| QCOM | 10-Q | 676 | 99.84% | 99.51% | arelle-null:14, text-diff:3 |
| TXN | 10-Q | 700 | 99.84% | 99.53% | arelle-null:6, text-diff:3 |
| CB | 10-Q | 2556 | 99.96% | 99.53% | arelle-null:59, text-diff:11 |
| MRK | 10-Q | 1390 | 100.00% | 99.53% | arelle-null:29, text-diff:6 |
| CAT | 10-Q | 2100 | 99.95% | 99.55% | arelle-null:40, text-diff:9 |
| HD | 10-Q | 515 | 100.00% | 99.55% | arelle-null:12, text-diff:2 |
| NVDA | 10-Q | 803 | 100.00% | 99.59% | arelle-null:24, text-diff:3 |
| AXP | 10-Q | 1378 | 99.92% | 99.60% | arelle-null:35, text-diff:5 |
| CVS | 10-Q | 1217 | 99.91% | 99.64% | arelle-null:20, text-diff:4 |
| LLY | 10-Q | 625 | 100.00% | 99.65% | arelle-null:21, text-diff:2 |

## Failures / skips

- ASML: no-filing 
- CYATY: no-filing 
- ARM: no-filing 
- HSBC: no-filing 
- BABA: no-filing 
- AZN: no-filing 
- NVS: no-filing 
- RY: no-filing 
- KXIAY: no-filing 
- SHEL: no-filing 
- TM: no-filing 
- MUFG: no-filing 
- SAP: no-filing 
- RTNTF: no-filing 
- BHP: no-filing 
- TTE: no-filing 
- NVO: no-filing 
- TD: no-filing 
- SAN: no-filing 
- RIO: no-filing 
- DTEGY: no-filing 
- UBS: no-filing 
- SMERY: no-filing 
- BUD: no-filing 
- SMFG: no-filing 
- HTHIY: no-filing 
- BTI: no-filing 
- SONY: no-filing 
- BBVA: no-filing 
- UL: no-filing 
- PDD: no-filing 
- HDB: no-filing 

## T1 kill-criterion check (registered 2026-06-12, deadline 2026-06-26)
- fact agreement >= 99%: coverage PASS, value PASS
- speedup >= 10x: PASS (median 49.1x)