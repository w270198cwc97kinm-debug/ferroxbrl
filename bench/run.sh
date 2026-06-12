#!/usr/bin/env bash
# Arelle-vs-ferroxbrl correctness + speed harness (W0 DoD).
#
# Usage: bench/run.sh testdata/aapl-20240928.htm
#
# Honest-bench notes (reported, not hidden):
#  - Arelle loads + discovers the full DTS (taxonomy schemas); ferroxbrl parses
#    the instance only. Both cold and warm(cached-DTS) Arelle timings are shown.
#  - The compared task is "facts out of one filing", which is the user-facing
#    workload for bulk extraction pipelines.
set -euo pipefail
cd "$(dirname "$0")/.."

FILE="${1:?usage: bench/run.sh <filing.htm>}"
VENV=bench/.venv
OUT_FX=/tmp/ferrox-bench.json
OUT_AR=/tmp/arelle-oim.json

if [ ! -x "$VENV/bin/arelleCmdLine" ]; then
  echo "== setting up Arelle venv (one-time) =="
  uv venv --python 3.12 "$VENV" >/dev/null
  uv pip install --python "$VENV/bin/python" -q arelle-release
fi

echo "== build ferroxbrl (release) =="
cargo build --release -q

echo "== ferroxbrl =="
FX_MS=$("$VENV/bin/python" - "$FILE" <<'EOF'
import subprocess, sys, time
t = time.perf_counter()
subprocess.run(["./target/release/ferroxbrl", "extract", sys.argv[1], "-o", "/tmp/ferrox-bench.json"],
               check=True, stderr=subprocess.DEVNULL)
print(f"{(time.perf_counter()-t)*1000:.1f}")
EOF
)
echo "ferroxbrl: ${FX_MS} ms"

run_arelle() {
  "$VENV/bin/python" - "$FILE" <<'EOF'
import subprocess, sys, time
t = time.perf_counter()
r = subprocess.run(["bench/.venv/bin/arelleCmdLine", "-f", sys.argv[1],
                    "--plugins", "saveLoadableOIM",
                    "--saveLoadableOIM", "/tmp/arelle-oim.json",
                    "--logLevel", "error"],
                   capture_output=True, text=True)
if r.returncode != 0:
    sys.stderr.write(r.stderr[-2000:])
print(f"{(time.perf_counter()-t)*1000:.1f}")
EOF
}

echo "== arelle (run 1: cold DTS cache possible) =="
AR1_MS=$(run_arelle)
echo "arelle run1: ${AR1_MS} ms"
echo "== arelle (run 2: warm cache) =="
AR2_MS=$(run_arelle)
echo "arelle run2 (warm): ${AR2_MS} ms"

echo "== correctness: fact-set comparison =="
"$VENV/bin/python" bench/compare.py "$OUT_FX" "$OUT_AR" || true

python3 - "$FX_MS" "$AR2_MS" <<'EOF'
import sys
fx, ar = float(sys.argv[1]), float(sys.argv[2])
if fx > 0:
    print(f"\nspeedup vs warm-cache Arelle: {ar/fx:.1f}x  (ferroxbrl {fx:.1f}ms vs arelle {ar:.0f}ms)")
EOF
