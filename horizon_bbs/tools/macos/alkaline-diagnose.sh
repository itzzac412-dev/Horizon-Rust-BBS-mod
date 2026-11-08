#!/usr/bin/env bash
# alkaline-diagnose.sh - READ-ONLY diagnosis of what limits Horizon's FPS on macOS.
# Changes nothing. The only privileged step (--gpu, powermetrics) is opt-in.
#
# Usage: alkaline-diagnose.sh [--home PATH] [--gpu]
set -u

HOME_OVERRIDE=""; WANT_GPU=0
while [ $# -gt 0 ]; do
  case "$1" in
    --home) HOME_OVERRIDE="$2"; shift 2 ;;
    --gpu) WANT_GPU=1; shift ;;
    -h|--help) sed -n '2,6p' "$0"; exit 0 ;;
    *) echo "unknown option: $1"; exit 2 ;;
  esac
done

hr() { printf '\n== %s ==\n' "$1"; }

find_game_home() {
  [ -n "$HOME_OVERRIDE" ] && { echo "$HOME_OVERRIDE"; return; }
  [ -n "${HORIZON_HOME:-}" ] && [ -d "$HORIZON_HOME" ] && { echo "$HORIZON_HOME"; return; }
  # Documented default is ~/.local/share/horizon; macOS builds may differ, so
  # search a few bounded, read-only places for horizon-graphics.txt/options.txt.
  for base in "$HOME/.local/share/horizon" "$HOME/Library/Application Support"; do
    [ -d "$base" ] || continue
    hit="$(find "$base" -maxdepth 4 \( -name horizon-graphics.txt -o -name latest.log \) 2>/dev/null | head -1)"
    if [ -n "$hit" ]; then
      d="$(dirname "$hit")"; [ "$(basename "$d")" = logs ] && d="$(dirname "$d")"
      echo "$d"; return
    fi
  done
}

hr "Machine"
sw_vers 2>/dev/null | sed 's/^/  /'
sysctl -n machdep.cpu.brand_string 2>/dev/null | sed 's/^/  CPU: /'
echo "  Cores: $(sysctl -n hw.ncpu 2>/dev/null) logical"
echo "  RAM:   $(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 )) GB"

hr "Display (the refresh rate is the ceiling for UNIQUE frames shown)"
system_profiler SPDisplaysDataType 2>/dev/null | grep -iE "Chipset|Cores|Metal|Resolution|UI Looks|Refresh|Framerate" | sed 's/^ */  /'
echo "  (No refresh line = macOS did not report one; a MacBook Air panel is 60 Hz.)"

hr "Power and thermal"
pmset -g batt | head -1 | sed 's/^/  /'
lpm="$(pmset -g | awk '/lowpowermode/ {print $2}')"
echo "  Low Power Mode: ${lpm:-unknown} (1 = on, which throttles CPU/GPU)"
pmset -g therm 2>/dev/null | sed 's/^/  /'
echo "  (CPU_Speed_Limit < 100 or any thermal warning = throttling.)"

hr "Memory pressure"
memory_pressure 2>/dev/null | tail -1 | sed 's/^/  /'
vm_stat 2>/dev/null | awk '/Pages free|Pages active|Pages wired|Pages occupied by compressor|Swapouts/ {print "  " $0}'

hr "Horizon process"
PID="$(pgrep -x horizon-client | head -1 || true)"
if [ -n "$PID" ]; then
  ps -o pid,%cpu,%mem,rss,nice,etime,command -p "$PID" | sed 's/^/  /'
  echo "  Threads: $(ps -M -p "$PID" 2>/dev/null | tail -n +2 | wc -l | tr -d ' ')"
else
  echo "  horizon-client is not running (start the game for CPU/thread data)."
fi

GH="$(find_game_home || true)"
hr "Horizon config and the actual present mode"
if [ -z "${GH:-}" ]; then
  echo "  Could not find the game home. Re-run with --home /path/to/game/home"
  echo "  (the folder containing options.txt and logs/)."
else
  echo "  Game home: $GH"
  for f in options.txt horizon-graphics.txt; do
    p="$GH/$f"
    if [ -f "$p" ]; then
      echo "  -- $f"
      grep -E '^(enableVsync|maxFps|renderDistance|presentMode|framePacing|batteryMode|graphicsMode|fullscreen)' "$p" | sed 's/^/     /'
    else
      echo "  -- $f: not found"
    fi
  done
  log="$GH/logs/latest.log"
  if [ -f "$log" ]; then
    echo "  -- Surface line from the last run (ground truth for the cap):"
    grep -i "configuring the surface" "$log" | tail -1 | sed 's/^/     /'
    echo "     Fifo = vsync/refresh-locked. Immediate = uncapped by the display."
    echo "  -- GPU/adapter lines:"
    grep -iE "adapter|gpu|metal" "$log" | head -3 | sed 's/^/     /'
  else
    echo "  -- logs/latest.log not found"
  fi
fi

if [ "$WANT_GPU" = 1 ]; then
  hr "GPU/CPU power sample (needs sudo, 5 s, read-only)"
  echo "  powermetrics is Apple's own sampler; it needs root to read GPU counters."
  sudo powermetrics --samplers cpu_power,gpu_power -i 1000 -n 5 2>/dev/null \
    | grep -iE "GPU HW active|GPU Power|CPU Power|Package Power|thermal" | sed 's/^/  /'
fi

hr "How to read this"
cat <<'TXT'
  * Surface line says Fifo while enableVsync:false  -> the known Horizon bug
    (issues #1711, #11693). Fix: alkaline-fps-config.sh, then see docs/FPS.md.
  * Surface line says Immediate but FPS is still low -> workload-bound (render
    distance, GPU). Benchmark with alkaline-bench.sh at a lower distance.
  * CPU_Speed_Limit < 100 or Low Power Mode on     -> thermal/power throttling.
  * High memory pressure / swapouts                -> close other apps.
TXT
