package io.github.aloofbuckle.cv4android.vpn

import android.net.VpnService
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

class Cv4aVpnTileService : TileService() {
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
        if (Cv4aNetworkModeController.systemProxyState(this).active) {
            qsTile?.apply {
                state = Tile.STATE_UNAVAILABLE
                updateTile()
            }
            Cv4aNetworkModeController.stopSystemProxy(this)
            return
        }

        if (VpnService.prepare(this) != null) {
            Cv4aTileSupport.openApp(this)
            return
        }
        val startState = Cv4aVpnPersistence.load(this)
        if (startState == null) {
            Cv4aTileSupport.openApp(this)
            return
        }
        qsTile?.apply {
            state = Tile.STATE_UNAVAILABLE
            updateTile()
        }
        Cv4aNetworkModeController.startSystemProxy(this, startState)
    }

    override fun onDestroy() {
        Cv4aTileSupport.removeStateListener(stateListener)
        super.onDestroy()
    }

    private fun refreshTile() {
        val actual = Cv4aNetworkModeController.systemProxyState(this)
        qsTile?.apply {
            state = when {
                actual.active -> Tile.STATE_ACTIVE
                actual.phase == "starting" || actual.phase == "stopping" -> Tile.STATE_UNAVAILABLE
                else -> Tile.STATE_INACTIVE
            }
            updateTile()
        }
    }
}
