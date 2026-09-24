package io.github.aloofbuckle.cv4android.vpn

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.util.Log

class Cv4aVpnBootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent?) {
        if (intent?.action !in setOf(
                Intent.ACTION_BOOT_COMPLETED,
                Intent.ACTION_MY_PACKAGE_REPLACED,
            )
        ) {
            return
        }
        if (!Cv4aVpnPersistence.desired(context)) return
        if (VpnService.prepare(context) != null) {
            Log.w(TAG, "VPN restore skipped because permission is not granted")
            return
        }
        val state = Cv4aVpnPersistence.load(context) ?: return
        runCatching {
            val service = Cv4aVpnPersistence.startIntent(context, state)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(service)
            } else {
                context.startService(service)
            }
        }.onFailure { error ->
            Log.e(TAG, "Unable to restore Android VPN after boot", error)
        }
    }

    companion object {
        private const val TAG = "Cv4aVpnBootReceiver"
    }
}
