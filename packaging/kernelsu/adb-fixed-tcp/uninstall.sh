#!/system/bin/sh

LOG=/data/adb/adb-fixed-tcp.log
printf '[%s] uninstall: clearing fixed TCP ADB port\n' "$(date '+%Y-%m-%d %H:%M:%S')" >> "$LOG"
setprop persist.adb.tcp.port ''
setprop service.adb.tcp.port ''
setprop ctl.restart adbd
