package io.github.aloofbuckle.cv4android.vpn

import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

class Cv4aTunTileService : TileService() {
    companion object {
        private const val TAG = "Cv4aTunTile"
    }

    private val worker = Executors.newSingleThreadExecutor()
    private val toggleInFlight = AtomicBoolean(false)
    private val stateListener: () -> Unit = {
        mainExecutor.execute { refreshTile() }
    }

    override fun onStartListening() {
        super.onStartListening()
        Cv4aTileSupport.addStateListener(stateListener)
        refreshTile()
    }

    override fun onStopListening() {
        Cv4aTileSupport.removeStateListener(stateListener)
        super.onStopListening()
    }

    override fun onTileAdded() {
        super.onTileAdded()
        refreshTile()
    }

    override fun onClick() {
        super.onClick()
        if (!toggleInFlight.compareAndSet(false, true)) return
        worker.execute {
            runCatching {
                val current = Cv4aNetworkModeController.tunActive(this)
                val target = !current
                mainExecutor.execute { updateTile(target) }
                val active = Cv4aNetworkModeController.setTun(this, target)
                check(active == target) {
                    "TUN state did not reach requested value (requested=$target actual=$active)"
                }
                active
            }.onSuccess { active ->
                clearFailure()
                mainExecutor.execute {
                    toggleInFlight.set(false)
                    updateTile(active)
                    // The controller emits an event while the toggle is still in flight.
                    // Emit one more event only after the final state is committed so the
                    // foreground WebView and SystemUI both resync from the same actual state.
                    Cv4aTileSupport.notifyStateChanged(this)
                }
            }.onFailure { error ->
                recordFailure("toggle", error)
                val actual = runCatching {
                    Cv4aNetworkModeController.tunActive(this)
                }.getOrNull()
                mainExecutor.execute {
                    toggleInFlight.set(false)
                    if (actual != null) {
                        updateTile(actual)
                    } else {
                        updateTileUnavailable()
                        Cv4aTileSupport.openApp(this)
                    }
                    Cv4aTileSupport.notifyStateChanged(this)
                }
            }
        }
    }

    override fun onDestroy() {
        Cv4aTileSupport.removeStateListener(stateListener)
        worker.shutdownNow()
        super.onDestroy()
    }

    private fun refreshTile() {
        if (toggleInFlight.get()) return
        worker.execute {
            runCatching { Cv4aNetworkModeController.tunActive(this) }
                .onSuccess { active ->
                    clearFailure()
                    mainExecutor.execute {
                        if (!toggleInFlight.get()) updateTile(active)
                    }
                }
                .onFailure { error ->
                    recordFailure("refresh", error)
                    mainExecutor.execute {
                        if (!toggleInFlight.get()) updateTileUnavailable()
                    }
                }
        }
    }

    private fun updateTile(active: Boolean) {
        qsTile?.apply {
            state = if (active) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
            updateTile()
        }
    }

    private fun updateTileUnavailable() {
        qsTile?.apply {
            state = Tile.STATE_UNAVAILABLE
            updateTile()
        }
    }

    private fun recordFailure(stage: String, error: Throwable) {
        Log.e(TAG, "$stage failed", error)
        runCatching {
            File(filesDir, "tun-tile-error.txt").writeText(
                "$stage\n${error.stackTraceToString()}",
            )
        }
    }

    private fun clearFailure() {
        runCatching { File(filesDir, "tun-tile-error.txt").delete() }
    }
}
