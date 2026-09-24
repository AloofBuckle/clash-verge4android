package io.github.aloofbuckle.cv4android.vpn

import android.content.Context
import android.content.Intent

data class Cv4aVpnStartState(
    val configPath: String,
    val selectedJson: String,
    val stack: String,
    val mtu: Int,
    val ipv6: Boolean,
)

object Cv4aVpnPersistence {
    private const val PREFS = "cv4a_vpn"
    private const val KEY_DESIRED = "desired"
    private const val KEY_CONFIG_PATH = "config_path"
    private const val KEY_SELECTED_JSON = "selected_json"
    private const val KEY_STACK = "stack"
    private const val KEY_MTU = "mtu"
    private const val KEY_IPV6 = "ipv6"

    fun desired(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_DESIRED, false)

    fun setDesired(context: Context, desired: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_DESIRED, desired)
            .apply()
    }

    fun save(context: Context, state: Cv4aVpnStartState) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_DESIRED, true)
            .putString(KEY_CONFIG_PATH, state.configPath)
            .putString(KEY_SELECTED_JSON, state.selectedJson)
            .putString(KEY_STACK, state.stack)
            .putInt(KEY_MTU, state.mtu)
            .putBoolean(KEY_IPV6, state.ipv6)
            .apply()
    }

    fun load(context: Context): Cv4aVpnStartState? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val configPath = prefs.getString(KEY_CONFIG_PATH, null)?.takeIf { it.isNotBlank() }
            ?: return null
        return Cv4aVpnStartState(
            configPath = configPath,
            selectedJson = prefs.getString(KEY_SELECTED_JSON, "{}") ?: "{}",
            stack = prefs.getString(KEY_STACK, "mixed") ?: "mixed",
            mtu = prefs.getInt(KEY_MTU, 1500),
            ipv6 = prefs.getBoolean(KEY_IPV6, true),
        )
    }

    fun startIntent(context: Context, state: Cv4aVpnStartState): Intent =
        Intent(context, Cv4aVpnService::class.java).apply {
            action = Cv4aVpnService.ACTION_START
            putExtra(Cv4aVpnService.EXTRA_CONFIG_PATH, state.configPath)
            putExtra(Cv4aVpnService.EXTRA_SELECTED_JSON, state.selectedJson)
            putExtra(Cv4aVpnService.EXTRA_STACK, state.stack)
            putExtra(Cv4aVpnService.EXTRA_MTU, state.mtu)
            putExtra(Cv4aVpnService.EXTRA_IPV6, state.ipv6)
        }
}
