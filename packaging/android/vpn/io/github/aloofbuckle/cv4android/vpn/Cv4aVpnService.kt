package io.github.aloofbuckle.cv4android.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.LocalSocket
import android.net.LocalSocketAddress
import android.net.VpnService
import android.os.Build
import android.util.Log
import android.system.OsConstants
import io.github.aloofbuckle.cv4android.MainActivity
import io.github.aloofbuckle.cv4android.R
import io.github.oviron.libmihomo.Clash
import io.github.oviron.libmihomo.TunInterface
import org.json.JSONObject
import java.io.File
import java.net.InetAddress
import java.net.InetSocketAddress
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

class Cv4aVpnService : VpnService() {
    data class State(
        val active: Boolean = false,
        val phase: String = "stopped",
        val detail: String = "Android VPN is stopped",
        val bridgeAbi: Int = 0,
    )

    private val worker = Executors.newSingleThreadExecutor()
    @Volatile private var vpnFd: Int = -1
    private val tunCallback = object : TunInterface {
        override fun protect(fd: Int): Boolean = this@Cv4aVpnService.protect(fd)

        override fun resolverProcess(
            protocol: Int,
            source: String,
            target: String,
            uid: Int,
        ): String = resolveProcess(protocol, source, target, uid)
    }

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "onCreate")
        createNotificationChannel()
        publish(State(phase = "starting", detail = "Preparing Android VPN"))
        startForegroundCompat(buildNotification("Starting Android VPN…"))
        try {
            Clash.load(applicationInfo.nativeLibraryDir)
            publish(snapshot().copy(bridgeAbi = Clash.bridgeABI()))
            Log.i(TAG, "mihomo bridge loaded abi=${Clash.bridgeABI()}")
        } catch (t: Throwable) {
            Log.e(TAG, "mihomo bridge load failed", t)
            publish(State(phase = "error", detail = "Mihomo bridge load failed: ${t.message}"))
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            Cv4aVpnPersistence.setDesired(this, false)
            worker.execute {
                syncRootCoexistence(false)
                stopVpnInternal("Android VPN stopped")
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
            return START_NOT_STICKY
        }
        if (intent?.action != ACTION_START) return START_NOT_STICKY

        val configPath = intent.getStringExtra(EXTRA_CONFIG_PATH).orEmpty()
        val selectedJson = intent.getStringExtra(EXTRA_SELECTED_JSON) ?: "{}"
        val stack = intent.getStringExtra(EXTRA_STACK) ?: "mixed"
        val mtu = intent.getIntExtra(EXTRA_MTU, 1500).coerceIn(576, 65535)
        val ipv6 = intent.getBooleanExtra(EXTRA_IPV6, true)
        Cv4aVpnPersistence.save(
            this,
            Cv4aVpnStartState(configPath, selectedJson, stack, mtu, ipv6),
        )
        worker.execute {
            try {
                startVpn(configPath, selectedJson, stack, mtu, ipv6)
            } catch (t: Throwable) {
                syncRootCoexistence(false)
                stopVpnInternal("Android VPN failed: ${t.message}", error = true)
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
        }
        return START_STICKY
    }

    private fun startVpn(
        configPath: String,
        selectedJson: String,
        stack: String,
        mtu: Int,
        ipv6: Boolean,
    ) {
        Log.i(TAG, "startVpn config=$configPath stack=$stack mtu=$mtu ipv6=$ipv6")
        require(Clash.isLoaded()) { "Mihomo bridge is not loaded" }
        val config = File(configPath).canonicalFile
        require(config.isFile) { "VPN runtime config is missing" }
        require(config.name == "config.yaml") { "VPN runtime config must be named config.yaml" }
        val home = config.parentFile ?: error("VPN runtime config has no parent directory")

        stopVpnInternal("Restarting Android VPN")
        publish(snapshot().copy(active = false, phase = "starting", detail = "Loading Mihomo profile"))
        Clash.suspended(false)

        val init = JSONObject().apply {
            put("home-dir", home.absolutePath)
            put("version", Build.VERSION.SDK_INT)
        }.toString()
        val selected = JSONObject(selectedJson)
        val setup = JSONObject().apply { put("selected-map", selected) }.toString()
        val result = AtomicReference<String?>(null)
        val latch = CountDownLatch(1)
        Clash.quickSetup(init, setup) {
            result.set(it)
            latch.countDown()
        }
        check(latch.await(20, TimeUnit.SECONDS)) { "Mihomo initialization timed out" }
        val setupError = result.get().orEmpty()
        check(setupError.isEmpty()) { "Mihomo initialization failed: $setupError" }
        Log.i(TAG, "mihomo profile initialized")

        publish(snapshot().copy(phase = "starting", detail = "Creating Android VPN interface"))
        val builder = Builder()
            .setSession("Clash Verge for Android")
            .setMtu(mtu)
            .addAddress(IPV4_ADDRESS, IPV4_PREFIX)
            .addRoute("0.0.0.0", 0)
            .addDnsServer(IPV4_DNS)
            .setBlocking(false)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) builder.setMetered(false)

        val addresses = mutableListOf("$IPV4_ADDRESS/$IPV4_PREFIX")
        val dns = mutableListOf(IPV4_DNS)
        if (ipv6) {
            builder.addAddress(IPV6_ADDRESS, IPV6_PREFIX)
            builder.addRoute("::", 0)
            builder.addDnsServer(IPV6_DNS)
            addresses += "$IPV6_ADDRESS/$IPV6_PREFIX"
            dns += IPV6_DNS
        }
        val fd = builder.establish()?.detachFd()
            ?: error("Android refused to establish the VPN interface")
        vpnFd = fd
        Log.i(TAG, "VpnService established fd=$fd")

        publish(snapshot().copy(phase = "starting", detail = "Attaching Mihomo to Android VPN"))
        val tunError = Clash.startTUN(
            fd,
            tunCallback,
            "CV4A-VPN",
            stack.lowercase(),
            addresses.joinToString(","),
            dns.joinToString(","),
            mtu,
        )
        check(tunError.isEmpty()) { "Mihomo TUN start failed: $tunError" }
        Log.i(TAG, "mihomo startTUN invoked")
        val state = State(
            active = true,
            phase = "running",
            detail = "Android VPN is active",
            bridgeAbi = Clash.bridgeABI(),
        )
        publish(state)
        syncRootCoexistence(true)
        notifyForeground("Android VPN · Mihomo")
    }

    private fun stopVpnInternal(detail: String, error: Boolean = false) {
        try {
            if (Clash.isLoaded()) {
                Clash.stopTun()
                Clash.suspended(true)
            }
        } catch (_: Throwable) {
        }
        vpnFd = -1
        publish(
            State(
                active = false,
                phase = if (error) "error" else "stopped",
                detail = detail,
                bridgeAbi = if (Clash.isLoaded()) runCatching { Clash.bridgeABI() }.getOrDefault(0) else 0,
            ),
        )
    }

    override fun onRevoke() {
        Cv4aVpnPersistence.setDesired(this, false)
        worker.execute {
            syncRootCoexistence(false)
            stopVpnInternal("Android revoked VPN permission")
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
        super.onRevoke()
    }

    override fun onDestroy() {
        Log.i(TAG, "onDestroy")
        syncRootCoexistence(false)
        stopVpnInternal("Android VPN service stopped")
        worker.shutdownNow()
        super.onDestroy()
    }

    private fun resolveProcess(protocol: Int, source: String, target: String, uid: Int): String {
        var resolvedUid = uid
        if (resolvedUid < 0 && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val local = parseSocketAddress(source)
            val remote = parseSocketAddress(target)
            if (local != null && remote != null) {
                resolvedUid = runCatching {
                    getSystemService(ConnectivityManager::class.java)
                        .getConnectionOwnerUid(protocol, local, remote)
                }.getOrDefault(-1)
            }
        }
        if (resolvedUid < 0) return ""
        val packageName = packageManager.getPackagesForUid(resolvedUid)?.firstOrNull().orEmpty()
        return "$resolvedUid\n$packageName"
    }

    private fun parseSocketAddress(value: String): InetSocketAddress? = runCatching {
        val host: String
        val port: Int
        if (value.startsWith("[")) {
            val bracket = value.lastIndexOf(']')
            host = value.substring(1, bracket)
            port = value.substring(bracket + 2).toInt()
        } else {
            val split = value.lastIndexOf(':')
            host = value.substring(0, split)
            port = value.substring(split + 1).toInt()
        }
        InetSocketAddress(InetAddress.getByName(host), port)
    }.getOrNull()

    private fun syncRootCoexistence(enable: Boolean) {
        val socketPath = File(applicationInfo.dataDir, "run/root-agent.sock")
        if (!socketPath.exists()) return
        runCatching {
            LocalSocket().use { socket ->
                socket.connect(
                    LocalSocketAddress(
                        socketPath.absolutePath,
                        LocalSocketAddress.Namespace.FILESYSTEM,
                    ),
                )
                socket.soTimeout = 2_000
                val request = "{\"op\":\"set_vpn_coexistence\",\"enable\":$enable}\n"
                socket.outputStream.write(request.toByteArray(Charsets.UTF_8))
                socket.outputStream.flush()
                val reply = socket.inputStream.bufferedReader().readLine().orEmpty()
                if (!reply.contains("\"ok\":true")) {
                    error("root-agent rejected coexistence update: $reply")
                }
            }
            Log.i(TAG, "root coexistence desired=$enable")
        }.onFailure { error ->
            Log.w(TAG, "Unable to sync root coexistence desired=$enable", error)
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            NOTIFICATION_CHANNEL,
            "Clash Verge VPN",
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Android VPN status"
            setShowBadge(false)
        }
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    private fun buildNotification(text: String): Notification {
        val open = Intent(this, MainActivity::class.java)
        val pending = PendingIntent.getActivity(
            this,
            0,
            open,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, NOTIFICATION_CHANNEL)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("Clash Verge for Android")
            .setContentText(text)
            .setContentIntent(pending)
            .setOngoing(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .build()
    }

    private fun startForegroundCompat(notification: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun notifyForeground(text: String) {
        getSystemService(NotificationManager::class.java)
            .notify(NOTIFICATION_ID, buildNotification(text))
    }

    companion object {
        private const val TAG = "Cv4aVpnService"
        const val ACTION_START = "io.github.aloofbuckle.cv4android.vpn.START"
        const val ACTION_STOP = "io.github.aloofbuckle.cv4android.vpn.STOP"
        const val EXTRA_CONFIG_PATH = "configPath"
        const val EXTRA_SELECTED_JSON = "selectedJson"
        const val EXTRA_STACK = "stack"
        const val EXTRA_MTU = "mtu"
        const val EXTRA_IPV6 = "ipv6"

        private const val NOTIFICATION_CHANNEL = "cv4a-vpn"
        private const val NOTIFICATION_ID = 3022
        private const val IPV4_ADDRESS = "172.19.0.1"
        private const val IPV4_PREFIX = 30
        private const val IPV4_DNS = "172.19.0.2"
        private const val IPV6_ADDRESS = "fdfe:dcba:9876::1"
        private const val IPV6_PREFIX = 126
        private const val IPV6_DNS = "fdfe:dcba:9876::2"

        @Volatile private var state = State()

        fun snapshot(): State = state

        private fun publish(next: State) {
            state = next
        }
    }

    private fun publish(next: State) {
        Companion.publish(next)
        Cv4aTileSupport.notifyStateChanged(this)
    }
}
