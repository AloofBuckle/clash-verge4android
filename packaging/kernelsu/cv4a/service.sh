#!/system/bin/sh

MODDIR=${0%/*}
PKG=io.github.aloofbuckle.cv4android
ROOT=/data/adb/clash-verge4android
APP_DATA=/data/user/0/$PKG
RUN_DIR=$APP_DATA/run
AGENT=$ROOT/bin/cv4a-root-agent
CORE=$ROOT/bin/mihomo
CONFIG=$ROOT/config/runtime.yaml
AGENT_SOCKET=$RUN_DIR/root-agent.sock
LOG_DIR=$ROOT/logs
BOOT_LOG=$LOG_DIR/boot.log

mkdir -p "$LOG_DIR"
chmod 700 "$LOG_DIR" 2>/dev/null || true
exec >>"$BOOT_LOG" 2>&1

log() {
  echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

log "KernelSU boot service starting"

# The control sockets live in credential-encrypted app storage. Wait until
# Android has completed boot and user 0 CE storage is available.
i=0
while [ "$i" -lt 180 ]; do
  if [ "$(getprop sys.boot_completed)" = "1" ] && [ "$(getprop sys.user.0.ce_available)" = "true" ]; then
    break
  fi
  i=$((i + 1))
  sleep 1
done

if [ "$(getprop sys.user.0.ce_available)" != "true" ]; then
  log "user 0 credential-encrypted storage did not become available"
  exit 1
fi

if [ ! -d "$APP_DATA" ]; then
  log "app data directory is missing: $APP_DATA"
  exit 1
fi
if [ ! -x "$AGENT" ]; then
  log "root-agent is missing or not executable: $AGENT"
  exit 1
fi
if [ ! -x "$CORE" ]; then
  log "Mihomo is missing or not executable: $CORE"
  exit 1
fi
if [ ! -f "$CONFIG" ]; then
  log "runtime config is missing: $CONFIG"
  exit 1
fi

APP_UID=$(stat -c %u "$APP_DATA" 2>/dev/null)
SOCKET_CONTEXT=$(ls -Zd "$APP_DATA" 2>/dev/null | awk '{print $1}')
if [ -z "$APP_UID" ] || [ -z "$SOCKET_CONTEXT" ]; then
  log "failed to resolve app uid or SELinux context"
  exit 1
fi

mkdir -p "$RUN_DIR"
chown "$APP_UID:$APP_UID" "$RUN_DIR" 2>/dev/null || true
chmod 700 "$RUN_DIR" 2>/dev/null || true
chcon "$SOCKET_CONTEXT" "$RUN_DIR" 2>/dev/null || true
rm -f "$AGENT_SOCKET" "$RUN_DIR/mihomo.sock" "$ROOT/run/mihomo.pid"

# Match the runtime bootstrap used by the APK. The socket itself is additionally
# protected by the app-private path, owner UID and root-agent SO_PEERCRED check.
ROOT_TYPE=$(cut -d: -f3 /proc/self/attr/current 2>/dev/null | tr -d '\000')
KSUD=$(command -v ksud 2>/dev/null || true)
if [ -z "$KSUD" ] && [ -x /data/adb/ksu/bin/ksud ]; then
  KSUD=/data/adb/ksu/bin/ksud
fi
if [ -n "$KSUD" ] && [ -n "$ROOT_TYPE" ]; then
  "$KSUD" sepolicy patch "allow untrusted_app $ROOT_TYPE unix_stream_socket connectto" >/dev/null 2>&1 || true
fi

log "starting root-agent uid=$APP_UID context=$SOCKET_CONTEXT root_type=$ROOT_TYPE"
nohup "$AGENT" serve \
  --socket "$AGENT_SOCKET" \
  --peer-uid "$APP_UID" \
  --profiles "$APP_DATA/profiles.json" \
  --runtime-preferences "$APP_DATA/runtime-preferences.json" \
  --backup-settings "$APP_DATA/backup-settings.json" \
  --backup-dir "$APP_DATA/backups" \
  --socket-context "$SOCKET_CONTEXT" \
  --start-core \
  >>"$LOG_DIR/agent.log" 2>&1 </dev/null &
AGENT_PID=$!
log "root-agent pid=$AGENT_PID"

EXPECT_TUN=0
if awk '
  /^tun:[[:space:]]*$/ { in_tun=1; next }
  in_tun && /^[^[:space:]]/ { exit }
  in_tun && /^[[:space:]]+enable:[[:space:]]*true[[:space:]]*$/ { found=1; exit }
  END { exit found ? 0 : 1 }
' "$CONFIG"; then
  EXPECT_TUN=1
fi

i=0
while [ "$i" -lt 600 ]; do
  if [ "$EXPECT_TUN" -eq 1 ]; then
    if [ -d /sys/class/net/Mihomo ] && \
       /system/bin/ip rule show 2>/dev/null | grep -q '^8900:' && \
       /system/bin/ip route show table 3022 2>/dev/null | grep -q 'dev Mihomo'; then
      log "Mihomo TUN is active"
      exit 0
    fi
  elif [ -S "$RUN_DIR/mihomo.sock" ] && [ -f "$ROOT/run/mihomo.pid" ]; then
    CORE_PID=$(cat "$ROOT/run/mihomo.pid" 2>/dev/null)
    if [ -n "$CORE_PID" ] && kill -0 "$CORE_PID" 2>/dev/null; then
      log "Mihomo core is active"
      exit 0
    fi
  fi
  if ! kill -0 "$AGENT_PID" 2>/dev/null; then
    log "root-agent exited before Mihomo became active"
    exit 1
  fi
  i=$((i + 1))
  sleep 0.1
done

if [ "$EXPECT_TUN" -eq 1 ]; then
  log "timed out waiting for Mihomo TUN"
else
  log "timed out waiting for Mihomo core"
fi
exit 1
