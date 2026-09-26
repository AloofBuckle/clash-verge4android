package io.github.aloofbuckle.cv4android.vpn

import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.service.quicksettings.TileService
import io.github.aloofbuckle.cv4android.MainActivity
import java.util.concurrent.CopyOnWriteArraySet

object Cv4aTileSupport {
    const val ACTION_STATE_CHANGED = "io.github.aloofbuckle.cv4android.NETWORK_STATE_CHANGED"

    private val stateListeners = CopyOnWriteArraySet<() -> Unit>()
    private val mainHandler = Handler(Looper.getMainLooper())

    fun requestVpnTileUpdate(context: Context) {
        requestListeningState(context, Cv4aVpnTileService::class.java)
    }

    fun requestTunTileUpdate(context: Context) {
        if (!isTunTileRegistered(context)) return
        requestListeningState(context, Cv4aTunTileService::class.java)
    }

    fun requestTileUpdates(context: Context) {
        val appContext = context.applicationContext
        requestVpnTileUpdate(appContext)
        if (isTunTileRegistered(appContext)) {
            // Some SystemUI implementations coalesce back-to-back active-tile requests.
            // Put the second request on the next main-loop turn without polling.
            mainHandler.post { requestTunTileUpdate(appContext) }
        }
    }

    fun setTunTileRegistered(context: Context, registered: Boolean) {
        val appContext = context.applicationContext
        val component = ComponentName(appContext, Cv4aTunTileService::class.java)
        val targetState = if (registered) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        } else {
            PackageManager.COMPONENT_ENABLED_STATE_DISABLED
        }
        if (appContext.packageManager.getComponentEnabledSetting(component) != targetState) {
            appContext.packageManager.setComponentEnabledSetting(
                component,
                targetState,
                PackageManager.DONT_KILL_APP,
            )
        }
        if (registered) requestTunTileUpdate(appContext)
    }

    fun syncTunTileRegistration(context: Context): Boolean {
        val appContext = context.applicationContext
        val available = runCatching {
            Cv4aRootAgentClient.status(appContext)
            true
        }.getOrDefault(false)
        setTunTileRegistered(appContext, available)
        return available
    }

    private fun isTunTileRegistered(context: Context): Boolean {
        val component = ComponentName(context, Cv4aTunTileService::class.java)
        return when (context.packageManager.getComponentEnabledSetting(component)) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
            PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
            PackageManager.COMPONENT_ENABLED_STATE_DISABLED_USER,
            PackageManager.COMPONENT_ENABLED_STATE_DISABLED_UNTIL_USED -> false
            else -> runCatching {
                context.packageManager.getServiceInfo(component, 0).enabled
            }.getOrDefault(false)
        }
    }

    fun addStateListener(listener: () -> Unit) {
        stateListeners.add(listener)
    }

    fun removeStateListener(listener: () -> Unit) {
        stateListeners.remove(listener)
    }

    fun notifyStateChanged(context: Context) {
        requestTileUpdates(context)
        stateListeners.forEach { listener ->
            runCatching { listener() }
        }
        context.applicationContext.sendBroadcast(
            Intent(ACTION_STATE_CHANGED).setPackage(context.packageName),
        )
    }

    private fun requestListeningState(context: Context, serviceClass: Class<out TileService>) {
        runCatching {
            TileService.requestListeningState(
                context.applicationContext,
                ComponentName(context, serviceClass),
            )
        }
    }

    fun openApp(tileService: TileService) {
        val intent = Intent(tileService, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val pendingIntent = PendingIntent.getActivity(
                tileService,
                0,
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            tileService.startActivityAndCollapse(pendingIntent)
        } else {
            @Suppress("DEPRECATION")
            tileService.startActivityAndCollapse(intent)
        }
    }
}
