#!/system/bin/sh

# The module owns only boot integration. Keep the APK-managed runtime, profiles,
# logs and cached GeoSite data intact when the module is removed.
ROOT=/data/adb/clash-verge4android

if [ -f "$ROOT/run/mihomo.pid" ]; then
  PID=$(cat "$ROOT/run/mihomo.pid" 2>/dev/null)
  if [ -n "$PID" ]; then
    kill "$PID" 2>/dev/null || true
  fi
fi

pkill -f "$ROOT/bin/cv4a-root-agent" 2>/dev/null || true
