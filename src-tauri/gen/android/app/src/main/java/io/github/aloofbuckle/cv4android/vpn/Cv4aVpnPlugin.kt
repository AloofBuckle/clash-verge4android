package io.github.aloofbuckle.cv4android.vpn

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.VpnService
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.result.ActivityResult
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

@InvokeArg
class StartVpnArgs {
    lateinit var configPath: String
    var selectedMap: Map<String, String> = emptyMap()
    var stack: String = "mixed"
    var mtu: Int = 1500
    var ipv6: Boolean = true
}

@InvokeArg
class InstallApkArgs {
    lateinit var path: String
}

@InvokeArg
class OpenUrlArgs {
    lateinit var url: String
}

@InvokeArg
class SetTunArgs {
    var enable: Boolean = false
}

@InvokeArg
class SetTunTileRegisteredArgs {
    var registered: Boolean = false
}

@TauriPlugin
class Cv4aVpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingArgs: StartVpnArgs? = null
    private var pendingInstallPath: String? = null
    private val networkWorker = Executors.newSingleThreadExecutor()
    private val stateListener: () -> Unit = {
        activity.runOnUiThread {
            trigger(
                "stateChanged",
                JSObject().apply { put("source", "android") },
            )
        }
    }

    init {
        Cv4aTileSupport.addStateListener(stateListener)
        Cv4aTileSupport.requestVpnTileUpdate(activity)
        networkWorker.execute {
            Cv4aTileSupport.syncTunTileRegistration(activity)
        }
    }

    override fun onResume() {
        super.onResume()
        Cv4aTileSupport.notifyStateChanged(activity)
        networkWorker.execute {
            Cv4aTileSupport.syncTunTileRegistration(activity)
        }
    }

    override fun onDestroy(activity: AppCompatActivity) {
        Cv4aTileSupport.removeStateListener(stateListener)
        networkWorker.shutdownNow()
        super.onDestroy(activity)
    }

    @Command
    fun status(invoke: Invoke) {
        invoke.resolve(statusObject())
    }

    @Command
    fun setTun(invoke: Invoke) {
        val args = invoke.parseArgs(SetTunArgs::class.java)
        networkWorker.execute {
            runCatching { Cv4aNetworkModeController.setTun(activity, args.enable) }
                .onSuccess { active ->
                    invoke.resolve(JSObject().apply { put("active", active) })
                }
                .onFailure { error ->
                    invoke.reject(error.message ?: "Unable to change TUN mode")
                }
        }
    }

    @Command
    fun setTunTileRegistered(invoke: Invoke) {
        val args = invoke.parseArgs(SetTunTileRegisteredArgs::class.java)
        runCatching {
            Cv4aTileSupport.setTunTileRegistered(activity, args.registered)
        }.onSuccess {
            invoke.resolve()
        }.onFailure { error ->
            invoke.reject(error.message ?: "Unable to update TUN tile registration")
        }
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
            Cv4aNetworkModeController.startSystemProxy(activity, args.toStartState())
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
            Cv4aNetworkModeController.startSystemProxy(activity, args.toStartState())
            invoke.resolve(statusObject("starting"))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to start Android VPN")
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        try {
            Cv4aNetworkModeController.stopSystemProxy(activity)
            invoke.resolve(statusObject("stopping"))
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to stop Android VPN")
        }
    }

    @Command
    fun installApk(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(InstallApkArgs::class.java)
            val apk = File(args.path)
            if (!apk.isFile) {
                invoke.reject("Downloaded APK is missing")
                return
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
                !activity.packageManager.canRequestPackageInstalls()
            ) {
                pendingInstallPath = apk.absolutePath
                val permissionIntent = Intent(
                    Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:${activity.packageName}"),
                )
                startActivityForResult(invoke, permissionIntent, "packageInstallPermissionResult")
                return
            }
            launchPackageInstaller(apk)
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to install APK")
        }
    }

    @Command
    fun openUrl(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(OpenUrlArgs::class.java)
            val uri = Uri.parse(args.url)
            if (uri.scheme !in setOf("http", "https") || uri.host.isNullOrBlank()) {
                invoke.reject("Invalid external URL")
                return
            }
            activity.startActivity(Intent(Intent.ACTION_VIEW, uri))
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to open URL")
        }
    }

    @ActivityCallback
    fun packageInstallPermissionResult(invoke: Invoke, result: ActivityResult) {
        val path = pendingInstallPath
        pendingInstallPath = null
        if (path == null) {
            invoke.reject("Downloaded APK is missing")
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
        ) {
            invoke.reject("Package installation permission was not granted")
            return
        }
        try {
            launchPackageInstaller(File(path))
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Unable to install APK")
        }
    }

    private fun launchPackageInstaller(apk: File) {
        verifyUpdatePackage(apk)
        val authority = "${activity.packageName}.fileprovider"
        val uri = FileProvider.getUriForFile(activity, authority, apk)
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        activity.startActivity(intent)
    }

    private fun verifyUpdatePackage(apk: File) {
        val packageManager = activity.packageManager
        val archive = packageManager.getPackageArchiveInfo(
            apk.absolutePath,
            PackageManager.GET_SIGNING_CERTIFICATES,
        ) ?: throw IllegalArgumentException("Downloaded APK is invalid")
        if (archive.packageName != activity.packageName) {
            throw IllegalArgumentException("Downloaded APK package does not match this app")
        }
        val installed = packageManager.getPackageInfo(
            activity.packageName,
            PackageManager.GET_SIGNING_CERTIFICATES,
        )
        val installedHistory = installed.signingInfo?.signingCertificateHistory?.toSet().orEmpty()
        val archiveSigners = archive.signingInfo?.apkContentsSigners?.toSet().orEmpty()
        if (archiveSigners.isEmpty() || installedHistory.isEmpty() || archiveSigners.none(installedHistory::contains)) {
            throw SecurityException("Downloaded APK signature does not match this app")
        }
    }

    private fun StartVpnArgs.toStartState() = Cv4aVpnStartState(
        configPath = configPath,
        selectedJson = JSONObject(selectedMap).toString(),
        stack = stack,
        mtu = mtu,
        ipv6 = ipv6,
    )

    private fun statusObject(phaseOverride: String? = null): JSObject {
        val state = Cv4aNetworkModeController.systemProxyState(activity)
        return JSObject().apply {
            put("supported", true)
            put("permissionGranted", VpnService.prepare(activity) == null)
            put("active", state.active)
            put("phase", phaseOverride ?: state.phase)
            put("detail", state.detail)
            put("bridgeAbi", state.bridgeAbi)
        }
    }
}
