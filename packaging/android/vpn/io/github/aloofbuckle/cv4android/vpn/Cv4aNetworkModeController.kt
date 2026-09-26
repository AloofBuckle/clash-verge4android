package io.github.aloofbuckle.cv4android.vpn

import android.app.ActivityManager
import android.content.Context
import android.content.Intent

object Cv4aNetworkModeController {
    data class SystemProxyState(
        val active: Boolean,
        val phase: String,
        val detail: String,
        val bridgeAbi: Int,
    )

    fun tunActive(context: Context): Boolean =
        Cv4aRootAgentClient.status(context).transparentActive

    fun setTun(context: Context, enable: Boolean): Boolean {
        val status = Cv4aRootAgentClient.setTransparent(context, enable)
        Cv4aTileSupport.notifyStateChanged(context)
        return status.transparentActive
    }

    fun systemProxyState(context: Context): SystemProxyState {
        val state = Cv4aVpnService.snapshot()
        val running = context.getSystemService(ActivityManager::class.java)
            .getRunningServices(Int.MAX_VALUE)
            .any { it.service.className == Cv4aVpnService::class.java.name }
        val active = running && state.active
        val phase = when {
            active -> state.phase
            running -> state.phase
            else -> "stopped"
        }
        val detail = if (!running && state.active) {
            "Android VPN service is not running"
        } else {
            state.detail
        }
        return SystemProxyState(
            active = active,
            phase = phase,
            detail = detail,
            bridgeAbi = state.bridgeAbi,
        )
    }

    fun startSystemProxy(context: Context, state: Cv4aVpnStartState) {
        context.startForegroundService(Cv4aVpnPersistence.startIntent(context, state))
    }

    fun stopSystemProxy(context: Context) {
        context.startService(
            Intent(context, Cv4aVpnService::class.java).apply {
                action = Cv4aVpnService.ACTION_STOP
            },
        )
    }
}
