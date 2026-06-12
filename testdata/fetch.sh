#!/usr/bin/env bash
# Fetch the EDGAR test filings used by bench/run.sh (public domain documents).
# SEC requires a descriptive User-Agent.
set -euo pipefail
cd "$(dirname "$0")"
UA="ferroxbrl-bench (open source benchmark; github.com/w270198cwc97kinm-debug/ferroxbrl)"
BASE="https://www.sec.gov/Archives/edgar/data/320193/000032019324000123"
for f in aapl-20240928.htm aapl-20240928.xsd aapl-20240928_cal.xml \
         aapl-20240928_def.xml aapl-20240928_lab.xml aapl-20240928_pre.xml; do
  [ -f "$f" ] || curl -sS -L -H "User-Agent: $UA" -o "$f" "$BASE/$f"
done
echo "testdata ready"
