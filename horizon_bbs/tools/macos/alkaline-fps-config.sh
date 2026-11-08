#!/usr/bin/env bash
# alkaline-fps-config.sh - set Horizon's FPS/vsync options SAFELY, with backups.
#
# Edits only these keys, nothing else, and deletes nothing:
#   options.txt          enableVsync, maxFps
#   horizon-graphics.txt presentMode
# Default mode is a DRY RUN. Every apply makes a timestamped backup first.
#
# Usage:
#   alkaline-fps-config.sh [--home PATH] [--fps N] [--apply]   preview / apply
#   alkaline-fps-config.sh [--home PATH] --backup-only          just back up
#   alkaline-fps-config.sh [--home PATH] --restore [STAMP]      undo (default latest)
#   alkaline-fps-config.sh --list                               list backups
# --fps N: cap, 10..260 (260 = unlimited in vanilla's slider). Default 120.
set -eu

BACKUPS="${ALKALINE_BACKUPS:-$HOME/.alkaline-backups}"
FILES="options.txt horizon-graphics.txt"
HOME_DIR=""; FPS=120; MODE=dry; STAMP_ARG=latest

while [ $# -gt 0 ]; do
  case "$1" in
    --home) HOME_DIR="$2"; shift 2 ;;
    --fps) FPS="$2"; shift 2 ;;
    --apply) MODE=apply; shift ;;
    --backup-only) MODE=backup; shift ;;
    --restore) MODE=restore; if [ $# -gt 1 ] && [ "${2#--}" = "$2" ]; then STAMP_ARG="$2"; shift; fi; shift ;;
    --list) MODE=list; shift ;;
    -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

case "$FPS" in ''|*[!0-9]*) echo "--fps must be a whole number" >&2; exit 2 ;; esac
if [ "$FPS" -lt 10 ] || [ "$FPS" -gt 260 ]; then echo "--fps must be 10..260" >&2; exit 2; fi

if [ "$MODE" = list ]; then
  [ -d "$BACKUPS" ] && ls -1 "$BACKUPS" || echo "no backups yet"
  exit 0
fi

find_home() {
  [ -n "$HOME_DIR" ] && { echo "$HOME_DIR"; return; }
  [ -n "${HORIZON_HOME:-}" ] && [ -d "$HORIZON_HOME" ] && { echo "$HORIZON_HOME"; return; }
  for base in "$HOME/.local/share/horizon" "$HOME/Library/Application Support"; do
    [ -d "$base" ] || continue
    hit="$(find "$base" -maxdepth 4 -name horizon-graphics.txt 2>/dev/null | head -1)"
    [ -n "$hit" ] && { dirname "$hit"; return; }
  done
}

GH="$(find_home || true)"
[ -n "$GH" ] && [ -d "$GH" ] || { echo "Cannot find the game home. Pass --home PATH (folder with options.txt)." >&2; exit 1; }

if pgrep -x horizon-client >/dev/null 2>&1 && [ "$MODE" != backup ] && [ "$MODE" != dry ]; then
  echo "horizon-client is running. It rewrites these files when it exits, which would" >&2
  echo "undo this change. Quit the game fully, then run again." >&2
  exit 1
fi

# set_key FILE KEY VALUE -> writes a temp file next to FILE then renames it over.
# Existing key is replaced in place; a missing key is appended. Other lines untouched.
set_key() {
  f="$1"; k="$2"; v="$3"; tmp="$f.alkaline-new"
  if grep -q "^$k:" "$f" 2>/dev/null; then
    awk -v k="$k" -v v="$v" 'BEGIN{p=k ":"} index($0,p)==1 {print k ":" v; next} {print}' "$f" > "$tmp"
  else
    { cat "$f"; [ -n "$(tail -c1 "$f")" ] && echo; echo "$k:$v"; } > "$tmp"
  fi
  mv "$tmp" "$f"
}

get_key() { grep "^$2:" "$1" 2>/dev/null | head -1 | cut -d: -f2- || true; }

do_backup() {
  suffix="${1:-}"
  stamp="$(date +%Y%m%d-%H%M%S)$suffix"; dest="$BACKUPS/$stamp"
  mkdir -p "$dest"
  printf '%s\n' "$GH" > "$dest/ORIGIN"
  for f in $FILES; do
    [ -f "$GH/$f" ] && cp -p "$GH/$f" "$dest/$f"
  done
  echo "Backup: $dest"
}

case "$MODE" in
  backup) do_backup; exit 0 ;;
  restore)
    if [ "$STAMP_ARG" = latest ]; then STAMP_ARG="$(ls -1 "$BACKUPS" 2>/dev/null | grep -v -e -pre-restore | tail -1)"; fi
    src="$BACKUPS/$STAMP_ARG"
    [ -f "$src/ORIGIN" ] || { echo "No such backup: $STAMP_ARG (see --list)" >&2; exit 1; }
    GH="$(cat "$src/ORIGIN")"
    echo "Restoring from $src to $GH (current files are backed up first)"
    do_backup -pre-restore
    for f in $FILES; do [ -f "$src/$f" ] && cp -p "$src/$f" "$GH/$f"; done
    echo "Restored."; exit 0 ;;
esac

echo "Game home: $GH"
echo "Plan (cap = $FPS fps; vsync off; present mode immediate):"
for entry in "options.txt enableVsync false" "options.txt maxFps $FPS" "horizon-graphics.txt presentMode immediate"; do
  set -- $entry
  if [ -f "$GH/$1" ]; then
    printf '  %-22s %-12s %s -> %s\n' "$1" "$2" "$(get_key "$GH/$1" "$2")" "$3"
  else
    printf '  %-22s %-12s (file missing: will be SKIPPED, start the game once to create it)\n' "$1" "$2"
  fi
done

if [ "$MODE" = dry ]; then
  echo; echo "Dry run only. Nothing changed. Add --apply to write (after quitting the game)."
  exit 0
fi

do_backup
for entry in "options.txt enableVsync false" "options.txt maxFps $FPS" "horizon-graphics.txt presentMode immediate"; do
  set -- $entry
  [ -f "$GH/$1" ] && set_key "$GH/$1" "$2" "$3"
done
echo "Applied. Undo with: $0 --restore"
echo "Next: start the game, then follow docs/FPS.md (the startup-bug workaround)."
