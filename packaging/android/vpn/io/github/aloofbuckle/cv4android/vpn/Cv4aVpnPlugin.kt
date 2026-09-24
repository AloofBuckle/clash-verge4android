package io.github.aloofbuckle.cv4android.vpn

import android.app.Activity
import android.app.ActivityManager
import android.content.Intent
import android.net.VpnService
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject

@InvokeArg
class StartVpnArgs {
    lateinit var configPath: String
    var selectedMap: Map<String, String> = emptyMap()
    var stack: String = "mixed"
    var mtu: Int = 1500
    var ipv6: Boolean = true
}

@TauriPlugin
class Cv4aVpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingArgs: StartVpnArgs? = null

    @Command
    fun status(invoke: Invoke) {
        invoke.resolve(statusObject())
    }

    @Command
    fun start(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StartVpnArgs::class.java)
            val prepare = VpnService.prepare(activity)
            if (prepare != null) {
                pendingArgs = args
                startActivityForResult(invoke, prepare, "vpnPermissionResult")
                return
            }
            startService(args)
            invoke.resolve(statusObject("starting"))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to start Android VPN")
        }
    }

    @ActivityCallback
    fun vpnPermissionResult(invoke: Invoke, result: ActivityResult) {
        val args = pendingArgs
        pendingArgs = null
        if (result.resultCode != Activity.RESULT_OK || args == null) {
            invoke.reject("Android VPN permission was not granted")
            return
        }
        try {
            startService(args)
            invoke.resolve(statusObject("starting"))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to start Android VPN")
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        try {
            val intent = Intent(activity, Cv4aVpnService::class.java).apply {
                action = Cv4aVpnService.ACTION_STOP
            }
            activity.startService(intent)
            invoke.resolve(statusObject("stopping"))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to stop Android VPN")
        }
    }

    private fun startService(args: StartVpnArgs) {
        val intent = Intent(activity, Cv4aVpnService::class.java).apply {
            action = Cv4aVpnService.ACTION_START
            putExtra(Cv4aVpnService.EXTRA_CONFIG_PATH, args.configPath)
            putExtra(
                Cv4aVpnService.EXTRA_SELECTED_JSON,
                JSONObject(args.selectedMap).toString(),
            )
            putExtra(Cv4aVpnService.EXTRA_STACK, args.stack)
            putExtra(Cv4aVpnService.EXTRA_MTU, args.mtu)
            putExtra(Cv4aVpnService.EXTRA_IPV6, args.ipv6)
        }
        activity.startForegroundService(intent)
    }

    private fun statusObject(phaseOverride: String? = null): JSObject {
        val state = Cv4aVpnService.snapshot()
        val running = activity.getSystemService(ActivityManager::class.java)
            .getRunningServices(Int.MAX_VALUE)
            .any { it.service.className == Cv4aVpnService::class.java.name }
        val active = running && state.active
        val phase = when {
            phaseOverride != null -> phaseOverride
            active -> state.phase
            running -> state.phase
            else -> "stopped"
        }
        val detail = if (!running && state.active) {
            "Android VPN service is not running"
        } else {
            state.detail
        }
        return JSObject().apply {
            put("supported", true)
            put("permissionGranted", VpnService.prepare(activity) == null)
            put("active", active)
            put("phase", phase)
            put("detail", detail)
            put("bridgeAbi", state.bridgeAbi)
        }
    }
}
