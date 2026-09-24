#!/system/bin/sh

PORT=40000
LOG=/data/adb/adb-fixed-tcp.log

log_msg() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >> "$LOG"
}

# Wait until Android's property/settings services are usable.
i=0
while [ "$i" -lt 120 ]; do
  if [ "$(getprop sys.boot_completed)" = "1" ]; then
    break
  fi
  sleep 1
  i=$((i + 1))
done

log_msg "configuring authenticated TCP ADB on port $PORT"

# Keep normal ADB enabled and make the classic TCP listener persistent.
settings put global adb_enabled 1
setprop persist.adb.tcp.port "$PORT"
setprop service.adb.tcp.port "$PORT"

# Disable Android 11+'s separate random-port TLS Wireless debugging path.
# The fixed TCP listener is authenticated by the normal ADB RSA key store
# (ro.adb.secure remains unchanged).
settings put global adb_wifi_enabled 0
setprop persist.adb.tls_server.enable 0

# Restart adbd once after all properties are final. This guarantees the new
# process starts with port 40000 and without the random TLS listener.
setprop ctl.restart adbd

i=0
while [ "$i" -lt 30 ]; do
  if ss -ltn 2>/dev/null | grep -q ":$PORT "; then
    break
  fi
  sleep 1
  i=$((i + 1))
done

if ss -ltn 2>/dev/null | grep -q ":$PORT "; then
  log_msg "TCP ADB listening on $PORT; random TLS Wireless debugging disabled"
else
  log_msg "warning: TCP ADB port $PORT was not observed after restart"
fi
