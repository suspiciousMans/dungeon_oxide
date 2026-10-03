#!/usr/bin/env bash
# Proves a run is winnable: the autopilot must reach MAX_DEPTH and set won=true.
# Pass bar: "escaped" in the output, zero panics, exit 0.
set -uo pipefail
cd "$(dirname "$0")/.."
BIN=./target/debug/dungeon_oxide.exe
[ -x "$BIN" ] || { echo "FAIL: build first (cargo build)"; exit 1; }

fails=0
for seed in 1 42 777 1234 2024; do
  out=$(DUNGEON_AUTOPILOT=1 DUNGEON_SEED=$seed RUST_LOG=info \
        timeout 120 "$BIN" 2>&1)
  panics=$(printf '%s' "$out" | grep -c 'panic')
  if printf '%s' "$out" | grep -q 'escaped'; then
    echo "seed $seed: WON   (panics=$panics)"
  else
    echo "seed $seed: LOST  (panics=$panics) $(printf '%s' "$out" | grep 'run ended' | tail -1)"
    fails=$((fails+1))
  fi
  [ "$panics" -eq 0 ] || { echo "  seed $seed had $panics panics"; fails=$((fails+1)); }
done
[ "$fails" -eq 0 ] && echo "ALL SEEDS WON" || echo "FAIL: $fails problem(s)"
exit $fails