#!/usr/bin/env bash
# alkaline-bench.sh - MEASURE achieved FPS at 60/90/120/144 targets with Horizon's
# own benchmark command (horizon-ctl fly), instead of trusting the requested cap.
#
# It uses only commands from Horizon's documented agent-control interface:
#   launch, options-set, fly, quit. `fly` replies with avg / 1%-low FPS, frame-time
# percentiles and CPU/GPU time per frame. Output is saved RAW (one JSON line per
# run); field names are not parsed because they are not documented here.
#
# NOTE: options-set rewrites options.txt (maxFps, enableVsync, renderDistance).
# This script backs the files up first; undo with alkaline-fps-config.sh --restore.
#
# Usage: alkaline-bench.sh --label before|after [--rd N] [--targets "60 90 120 144"]
#                          [--seconds S] [--world NAME] [--ctl PATH] [--attached]
#   --attached  use an already-running game started with --control (no launch/quit)
set -eu

LABEL=run; RD=16; TARGETS="60 90 120 144"; SECONDS_PER=30; WORLD=AlkalineBench
CTL="${HORIZON_CTL:-horizon-ctl}"; ATTACHED=0
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${ALKALINE_BENCH_DIR:-$HOME/.alkaline-bench}"

while [ $# -gt 0 ]; do
  case "$1" in
    --label) LABEL="$2"; shift 2 ;;
    --rd) RD="$2"; shift 2 ;;
    --targets) TARGETS="$2"; shift 2 ;;
    --seconds) SECONDS_PER="$2"; shift 2 ;;
    --world) WORLD="$2"; shift 2 ;;
    --ctl) CTL="$2"; shift 2 ;;
    --attached) ATTACHED=1; shift ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

command -v "$CTL" >/dev/null 2>&1 || { echo "horizon-ctl not found. Pass --ctl /path/to/horizon-ctl (shipped beside horizon-client)." >&2; exit 1; }

echo "Backing up Horizon config first..."
"$HERE/alkaline-fps-config.sh" --backup-only

mkdir -p "$OUT"; FILE="$OUT/$LABEL-$(date +%Y%m%d-%H%M%S).jsonl"
echo "Results: $FILE"
echo "Fixed workload: world '$WORLD', render distance $RD, ${SECONDS_PER}s flight per target."
echo "Plug in the charger, close other apps, and do not touch the Mac while it runs."

if [ "$ATTACHED" = 0 ]; then
  "$CTL" launch --world "$WORLD" --seed 1234 --creative --render-distance "$RD" --wait
fi

# Same camera path every run so before/after are comparable. Chunks must be loaded
# first: fly one throwaway pass to warm up, then measure.
"$CTL" options-set renderDistance "$RD" || true
"$CTL" fly 0 110 0 160 110 0 15 >/dev/null || true

for T in $TARGETS; do
  echo "--- target $T fps (vsync off)"
  "$CTL" options-set enableVsync false >/dev/null
  "$CTL" options-set maxFps "$T" >/dev/null
  sleep 2
  RESULT="$("$CTL" fly 0 110 0 160 110 0 "$SECONDS_PER")"
  printf '{"label":"%s","target":%s,"rd":%s,"result":%s}\n' "$LABEL" "$T" "$RD" "$RESULT" | tee -a "$FILE"
done

[ "$ATTACHED" = 0 ] && "$CTL" quit || true
echo
echo "Done. Compare achieved vs target in $FILE (achieved avg FPS must be close to,"
echo "and never above, the target; with a 60 Hz panel only 60 unique frames/s reach the screen)."
echo "Restore your original settings: $HERE/alkaline-fps-config.sh --restore"
