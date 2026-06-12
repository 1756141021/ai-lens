#!/usr/bin/env bash
# X11 smoke inside the build container: boot the real binary under Xvfb, prove
# the settings window actually renders pixels, dump frames for human review.
# Output lands in /work/.smoke/ (bind-mounted back to the host repo).
set -u
OUT=/work/.smoke
mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/report.txt

export DISPLAY=:99
export WEBKIT_DISABLE_DMABUF_RENDERER=1
Xvfb :99 -screen 0 1280x800x24 &
XVFB=$!
sleep 1
eval "$(dbus-launch --sh-syntax)"

BIN=/work/src-tauri/target/release/ai-lens
{
  echo "== smoke $(date -u +%H:%M:%S) =="
  "$BIN" --settings &
  APP=$!
  sleep 10

  echo "-- window tree --"
  xwininfo -root -tree | sed -n '1,40p'

  import -window root "$OUT/settings.png" && echo "settings.png dumped"

  echo "-- second instance --capture (single-instance forward) --"
  "$BIN" --capture || true
  sleep 5
  import -window root "$OUT/capture.png" && echo "capture.png dumped"

  echo "-- app still alive: $(kill -0 $APP 2>/dev/null && echo yes || echo NO) --"
  echo "-- config dir --"
  ls -la /root/.config/ai-lens/ 2>/dev/null || echo "(none)"

  kill $APP 2>/dev/null
} 2>&1 | tee "$OUT/report.txt"
kill $XVFB 2>/dev/null
