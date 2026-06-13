#!/usr/bin/env python3
"""100-filing EDGAR parity run (kill criterion T1).

For ~100 large filers: fetch the latest 10-Q (fallback 10-K) inline XBRL
primary document + taxonomy companions, extract facts with both ferroxbrl and
Arelle (saveLoadableOIM), compare fact sets, and aggregate:

  - per-filing key coverage and value match (vs Arelle as reference)
  - wall-clock speedup (CLI process vs arelleCmdLine process, warm cache)
  - failure taxonomy (download / arelle-error / degenerate-DTS / ferrox-error)

Resumable: downloads and OIM exports are cached on disk; re-runs skip work.
Writes bench/results-100.json and bench/results-100.md.

SEC fair-access: descriptive User-Agent, sequential requests with small delay.
"""
import json
import os
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from compare import load_ferrox, load_arelle, fact_key, values_match  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "bench" / "corpus"
FERROX = ROOT / "target" / "release" / "ferroxbrl"
ARELLE = ROOT / "bench" / ".venv" / "bin" / "arelleCmdLine"
UA = "ferroxbrl-bench (open source benchmark; contact: 261541876+w270198cwc97kinm-debug@users.noreply.github.com)"
N_TARGET = int(os.environ.get("N_TARGET", "100"))
DELAY = 0.15  # seconds between SEC requests


def fetch(url, binary=False, retries=3):
    for attempt in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=60) as r:
                data = r.read()
            time.sleep(DELAY)
            return data if binary else json.loads(data)
        except Exception as e:
            if attempt == retries - 1:
                raise
            time.sleep(2.0 * (attempt + 1))


def list_target_filings():
    """Yield dicts {ticker, cik, form, accession, primary} for latest 10-Q/10-K."""
    tickers = fetch("https://www.sec.gov/files/company_tickers.json")
    # ordered dict keyed by index; roughly market-cap ordered
    companies = [tickers[k] for k in sorted(tickers, key=lambda x: int(x))]
    seen_cik = set()
    out = []
    for c in companies:
        cik = int(c["cik_str"])
        if cik in seen_cik:
            continue
        seen_cik.add(cik)
        out.append({"ticker": c["ticker"], "cik": cik, "title": c.get("title", "")})
        if len(out) >= 160:  # spares for failures
            break
    return out


def latest_filing(cik):
    sub = fetch(f"https://data.sec.gov/submissions/CIK{cik:010d}.json")
    recent = sub.get("filings", {}).get("recent", {})
    forms = recent.get("form", [])
    accs = recent.get("accessionNumber", [])
    prims = recent.get("primaryDocument", [])
    dates = recent.get("filingDate", [])
    best = None
    for form_want in ("10-Q", "10-K"):
        for i, f in enumerate(forms):
            if f == form_want and prims[i].endswith(".htm"):
                best = {
                    "form": f,
                    "accession": accs[i].replace("-", ""),
                    "primary": prims[i],
                    "date": dates[i],
                }
                break
        if best:
            break
    return best


def download_filing(cik, info, dest: Path):
    dest.mkdir(parents=True, exist_ok=True)
    base = f"https://www.sec.gov/Archives/edgar/data/{cik}/{info['accession']}"
    # primary doc
    prim = dest / Path(info["primary"]).name
    if not prim.exists():
        prim.write_bytes(fetch(f"{base}/{info['primary']}", binary=True))
    # companion taxonomy files from index.json
    idx_path = dest / "_index.json"
    if idx_path.exists():
        idx = json.loads(idx_path.read_text())
    else:
        idx = fetch(f"{base}/index.json")
        idx_path.write_text(json.dumps(idx))
    pat = re.compile(r"(\.xsd|_cal\.xml|_def\.xml|_lab\.xml|_pre\.xml)$")
    for item in idx.get("directory", {}).get("item", []):
        name = item.get("name", "")
        if pat.search(name):
            f = dest / name
            if not f.exists():
                f.write_bytes(fetch(f"{base}/{name}", binary=True))
    return prim


def run_ferrox(prim: Path, out: Path):
    t = time.perf_counter()
    r = subprocess.run(
        [str(FERROX), "extract", str(prim), "-o", str(out)],
        capture_output=True, text=True,
    )
    ms = (time.perf_counter() - t) * 1000
    return r.returncode, ms, r.stderr[-500:]


def run_arelle(prim: Path, out: Path):
    t = time.perf_counter()
    r = subprocess.run(
        [str(ARELLE), "-f", str(prim), "--plugins", "saveLoadableOIM",
         "--saveLoadableOIM", str(out), "--logLevel", "error"],
        capture_output=True, text=True,
    )
    ms = (time.perf_counter() - t) * 1000
    return r.returncode, ms, (r.stderr or r.stdout)[-500:]


def compare_files(fx_path, ar_path):
    fx = load_ferrox(fx_path)
    ar = load_arelle(ar_path)
    if not ar:
        return None
    # degenerate DTS: facts without concept
    n_no_concept = sum(1 for f in ar if not f["concept"])
    if n_no_concept > len(ar) * 0.5:
        return {"status": "arelle-degenerate"}
    arm, fxm = {}, {}
    for f in ar:
        arm.setdefault(fact_key(f), []).append(f)
    for f in fx:
        fxm.setdefault(fact_key(f), []).append(f)
    common = [k for k in arm if k in fxm]
    matched = 0
    compared = 0
    arelle_null = 0
    kinds = {}
    for k in common:
        ok, kind = values_match(fxm[k][0]["value"], arm[k][0]["value"])
        kinds[("match" if ok else "MISMATCH", kind)] = kinds.get(("match" if ok else "MISMATCH", kind), 0) + 1
        if kind == "arelle-null":
            arelle_null += 1
            continue
        compared += 1
        if ok:
            matched += 1
    return {
        "status": "ok",
        "arelle_facts": len(ar),
        "ferrox_facts": len(fx),
        "keys_arelle": len(arm),
        "keys_common": len(common),
        "coverage": len(common) / max(1, len(arm)),
        "value_matched": matched,
        "value_compared": compared,
        "value_rate": matched / max(1, compared),
        "arelle_null": arelle_null,
        "mismatch_kinds": {f"{a}:{b}": v for (a, b), v in kinds.items() if a == "MISMATCH"},
    }


def main():
    CORPUS.mkdir(parents=True, exist_ok=True)
    results = []
    companies = list_target_filings()
    done = 0
    for c in companies:
        if done >= N_TARGET:
            break
        tag = f"{c['ticker']}-{c['cik']}"
        rec = {"ticker": c["ticker"], "cik": c["cik"], "title": c["title"]}
        try:
            info = latest_filing(c["cik"])
            if not info:
                rec["status"] = "no-filing"
                results.append(rec)
                continue
            rec.update(form=info["form"], date=info["date"])
            dest = CORPUS / tag
            prim = download_filing(c["cik"], info, dest)
            fx_out = dest / "ferrox.json"
            ar_out = dest / "arelle-oim.json"
            rc, fx_ms, err = run_ferrox(prim, fx_out)
            if rc != 0:
                rec.update(status="ferrox-error", error=err)
                results.append(rec)
                print(f"[{done}] {tag}: FERROX-ERROR {err[:120]}", flush=True)
                continue
            rec["ferrox_ms"] = round(fx_ms, 1)
            if ar_out.exists() and ar_out.stat().st_size > 100:
                ar_ms = None
            else:
                rc, ar_ms, err = run_arelle(prim, ar_out)
                if rc != 0 or not ar_out.exists():
                    rec.update(status="arelle-error", error=err)
                    results.append(rec)
                    print(f"[{done}] {tag}: ARELLE-ERROR {err[:120]}", flush=True)
                    continue
                rec["arelle_ms"] = round(ar_ms, 1)
            cmp_res = compare_files(fx_out, ar_out)
            if cmp_res is None or cmp_res.get("status") != "ok":
                rec["status"] = (cmp_res or {}).get("status", "compare-error")
                results.append(rec)
                print(f"[{done}] {tag}: {rec['status']}", flush=True)
                continue
            rec.update(cmp_res)
            if rec.get("arelle_ms") and rec.get("ferrox_ms"):
                rec["speedup"] = round(rec["arelle_ms"] / max(0.1, rec["ferrox_ms"]), 1)
            results.append(rec)
            done += 1
            print(
                f"[{done}] {tag} {info['form']}: facts {rec['ferrox_facts']}/{rec['arelle_facts']} "
                f"cov {rec['coverage']*100:.2f}% val {rec['value_rate']*100:.2f}% "
                f"fx {rec.get('ferrox_ms','-')}ms ar {rec.get('arelle_ms','-')}ms",
                flush=True,
            )
        except Exception as e:
            rec.update(status="error", error=str(e)[:300])
            results.append(rec)
            print(f"[{done}] {tag}: EXC {e}", flush=True)
        # periodic checkpoint
        (ROOT / "bench" / "results-100.json").write_text(json.dumps(results, indent=1))

    (ROOT / "bench" / "results-100.json").write_text(json.dumps(results, indent=1))

    ok = [r for r in results if r.get("status") == "ok"]
    fails = [r for r in results if r.get("status") != "ok"]
    if ok:
        total_keys = sum(r["keys_arelle"] for r in ok)
        total_common = sum(r["keys_common"] for r in ok)
        total_cmp = sum(r["value_compared"] for r in ok)
        total_match = sum(r["value_matched"] for r in ok)
        speedups = sorted(r["speedup"] for r in ok if "speedup" in r)
        med_speed = speedups[len(speedups) // 2] if speedups else None
        fx_total = sum(r["ferrox_ms"] for r in ok if "ferrox_ms" in r)
        ar_total = sum(r["arelle_ms"] for r in ok if "arelle_ms" in r)
        lines = [
            "# ferroxbrl — 100-filing EDGAR parity run",
            "",
            f"- filings compared OK: **{len(ok)}** (failures/skips: {len(fails)})",
            f"- aggregate key coverage: **{100*total_common/max(1,total_keys):.3f}%** ({total_common}/{total_keys})",
            f"- aggregate value match: **{100*total_match/max(1,total_cmp):.3f}%** ({total_match}/{total_cmp})",
            f"- median per-filing speedup (CLI vs warm Arelle): **{med_speed}x**"
            if med_speed else "- speedup: n/a",
            f"- total wall: ferroxbrl {fx_total/1000:.1f}s vs Arelle {ar_total/1000:.1f}s",
            "",
            "## Worst filings by value match",
            "",
            "| ticker | form | facts | coverage | value match | top mismatch kinds |",
            "|---|---|---|---|---|---|",
        ]
        for r in sorted(ok, key=lambda r: r["value_rate"])[:15]:
            kinds = ", ".join(f"{k.split(':')[1]}:{v}" for k, v in
                              sorted(r.get("mismatch_kinds", {}).items(), key=lambda x: -x[1])[:3])
            lines.append(
                f"| {r['ticker']} | {r.get('form','')} | {r['arelle_facts']} | "
                f"{r['coverage']*100:.2f}% | {r['value_rate']*100:.2f}% | {kinds} |"
            )
        if fails:
            lines += ["", "## Failures / skips", ""]
            for r in fails:
                lines.append(f"- {r.get('ticker')}: {r.get('status')} {str(r.get('error',''))[:140]}")
        # T1 verdict
        cov = total_common / max(1, total_keys)
        val = total_match / max(1, total_cmp)
        lines += [
            "",
            "## T1 kill-criterion check (registered 2026-06-12, deadline 2026-06-26)",
            f"- fact agreement >= 99%: coverage {'PASS' if cov >= 0.99 else 'FAIL'}, "
            f"value {'PASS' if val >= 0.99 else 'FAIL'}",
            f"- speedup >= 10x: {'PASS' if (med_speed or 0) >= 10 else 'FAIL'} (median {med_speed}x)",
        ]
        (ROOT / "bench" / "results-100.md").write_text("\n".join(lines))
        print("\n".join(lines[:12]))
    else:
        print("no successful comparisons")


if __name__ == "__main__":
    main()
