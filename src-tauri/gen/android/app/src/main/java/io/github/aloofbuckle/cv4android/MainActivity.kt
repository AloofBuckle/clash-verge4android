package io.github.aloofbuckle.cv4android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.res.Configuration
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import io.github.aloofbuckle.cv4android.vpn.Cv4aTileSupport

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null
  private val nativeStateReceiver = object : BroadcastReceiver() {
    override fun onReceive(context: Context?, intent: Intent?) {
      if (intent?.action == Cv4aTileSupport.ACTION_STATE_CHANGED) {
        dispatchWebEvent("cv4a-native-state")
      }
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    ContextCompat.registerReceiver(
      this,
      nativeStateReceiver,
      IntentFilter(Cv4aTileSupport.ACTION_STATE_CHANGED),
      ContextCompat.RECEIVER_NOT_EXPORTED,
    )
    if (BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true)
    updateSystemBars()
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
      WindowInsetsCompat.CONSUMED
    }
    ViewCompat.requestApplyInsets(content)
    updateSystemBars()
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    updateSystemBars()
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) {
      Cv4aTileSupport.requestTileUpdates(this)
      dispatchWebEvent("cv4a-native-focus")
    }
  }

  override fun onDestroy() {
    unregisterReceiver(nativeStateReceiver)
    appWebView = null
    super.onDestroy()
  }

  private fun dispatchWebEvent(name: String) {
    appWebView?.post {
      appWebView?.evaluateJavascript(
        "window.dispatchEvent(new Event('$name'))",
        null,
      )
    }
  }

  private fun updateSystemBars() {
    enableEdgeToEdge()
    val dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    val controller = WindowCompat.getInsetsController(window, window.decorView)
    controller.isAppearanceLightStatusBars = !dark
    controller.isAppearanceLightNavigationBars = !dark
    findViewById<View>(android.R.id.content)?.setBackgroundColor(
      Color.parseColor(if (dark) "#11131b" else "#f6f5fa")
    )
  }
}
