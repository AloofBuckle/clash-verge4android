import { createHash } from 'node:crypto'
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'

const bridgeCommit = '1f6d56cea98eac9004473eed09b2eef54bd5a1c2'
const mihomoVersion = 'v1.19.31'
const kotlinVersion = '1.9.25'
const loaderPatchVersion = 1
const bridgePatchVersion = 2
const bridgePatch = path.resolve('packaging/android/vpn/libmihomo-cv4a.patch')
const root = path.resolve('.local-artifacts/android-vpn')
const source = path.join(root, 'libmihomo-android')
const aar = path.join(root, `libmihomo-android-${mihomoVersion}-arm64.aar`)
const metadata = path.join(root, 'build.json')
const generatedApp = path.resolve('src-tauri/gen/android/app')

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    stdio: 'inherit',
    ...options,
    env: { ...process.env, ...(options.env ?? {}) },
  })
  if (result.status !== 0)
    throw new Error(`${command} exited with status ${result.status}`)
}

function sha256(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex')
}

function validCachedAar() {
  if (!existsSync(aar) || !existsSync(metadata)) return false
  try {
    const value = JSON.parse(readFileSync(metadata, 'utf8'))
    return (
      value.bridgeCommit === bridgeCommit &&
      value.mihomoVersion === mihomoVersion &&
      value.kotlinVersion === kotlinVersion &&
      value.loaderPatchVersion === loaderPatchVersion &&
      value.bridgePatchVersion === bridgePatchVersion &&
      value.aarSha256 === sha256(aar)
    )
  } catch {
    return false
  }
}

function buildBridge({ ndk, java }) {
  mkdirSync(root, { recursive: true })
  if (!existsSync(path.join(source, '.git'))) {
    rmSync(source, { recursive: true, force: true })
    run('git', ['clone', 'https://github.com/oviron/libmihomo-android.git', source])
  }
  run('git', ['fetch', '--depth', '1', 'origin', bridgeCommit], { cwd: source })
  run('git', ['checkout', '--force', bridgeCommit], { cwd: source })
  run('git', ['clean', '-fdx'], { cwd: source })
  run('git', ['apply', bridgePatch], { cwd: source })

  const coreDir = path.join(source, 'src/main/jni/core')
  run('go', ['get', `github.com/metacubex/mihomo@${mihomoVersion}`], { cwd: coreDir })
  run('go', ['mod', 'tidy'], { cwd: coreDir })

  // Modern Android installs can mmap native libraries directly from the APK,
  // leaving applicationInfo.nativeLibraryDir empty. Keep the upstream
  // explicit-path loader for extracted installs, but fall back to the system
  // class loader when the .so files only exist inside the APK.
  const clashKt = path.join(
    source,
    'src/main/kotlin/io/github/oviron/libmihomo/Clash.kt',
  )
  let clashText = readFileSync(clashKt, 'utf8')
  clashText = clashText.replace(
    '            System.load(resolveLib(nativeLibDir, "libclash.so"))\n            System.load(resolveLib(nativeLibDir, "libmihomo-jni.so"))',
    `            val clashFile = File(nativeLibDir, "libclash.so")\n            val jniFile = File(nativeLibDir, "libmihomo-jni.so")\n            if (clashFile.isFile && jniFile.isFile) {\n                System.load(clashFile.absolutePath)\n                System.load(jniFile.absolutePath)\n            } else {\n                System.loadLibrary("clash")\n                System.loadLibrary("mihomo-jni")\n            }`,
  )
  if (!clashText.includes('System.loadLibrary("mihomo-jni")')) {
    throw new Error('Unable to patch libmihomo Android native loader')
  }
  writeFileSync(clashKt, clashText)

  const gradle = path.join(source, 'build.gradle.kts')
  let gradleText = readFileSync(gradle, 'utf8')
  gradleText = gradleText
    .replace(
      'id("org.jetbrains.kotlin.android") version "2.2.10"',
      `id("org.jetbrains.kotlin.android") version "${kotlinVersion}"`,
    )
    .replace('ndkVersion = "28.0.13004108"', 'ndkVersion = "29.0.14206865"')
    .replace(
      'abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")',
      'abiFilters += listOf("arm64-v8a")',
    )
  writeFileSync(gradle, gradleText)

  run('./build-native.sh', [], {
    cwd: source,
    env: { ANDROID_NDK: ndk, ABIS: 'arm64-v8a' },
  })
  run('./gradlew', ['assembleRelease', '--no-daemon'], {
    cwd: source,
    env: {
      ANDROID_HOME: process.env.ANDROID_HOME || '/opt/android-sdk',
      ANDROID_NDK: ndk,
      ...(java ? { JAVA_HOME: java, PATH: `${java}/bin:${process.env.PATH}` } : {}),
    },
  })
  const built = path.join(source, 'build/outputs/aar/libmihomo-android-release.aar')
  copyFileSync(built, aar)
  writeFileSync(
    metadata,
    `${JSON.stringify(
      {
        bridgeCommit,
        mihomoVersion,
        kotlinVersion,
        loaderPatchVersion,
        bridgePatchVersion,
        aarSha256: sha256(aar),
      },
      null,
      2,
    )}\n`,
  )
}

function patchGradle() {
  const file = path.join(generatedApp, 'build.gradle.kts')
  let text = readFileSync(file, 'utf8')
  const dependency = '    implementation(files("libs/libmihomo-android.aar"))\n'
  if (!text.includes('libmihomo-android.aar')) {
    text = text.replace('dependencies {\n', `dependencies {\n${dependency}`)
    writeFileSync(file, text)
  }
}

function patchManifest() {
  const file = path.join(generatedApp, 'src/main/AndroidManifest.xml')
  let text = readFileSync(file, 'utf8')
  for (const permission of [
    'android.permission.FOREGROUND_SERVICE',
    'android.permission.FOREGROUND_SERVICE_SPECIAL_USE',
    'android.permission.RECEIVE_BOOT_COMPLETED',
    'android.permission.REQUEST_INSTALL_PACKAGES',
  ]) {
    if (!text.includes(permission)) {
      text = text.replace(
        '    <uses-permission android:name="android.permission.INTERNET" />',
        `    <uses-permission android:name="android.permission.INTERNET" />\n    <uses-permission android:name="${permission}" />`,
      )
    }
  }
  if (!text.includes('android:name=".vpn.Cv4aVpnService"')) {
    const service = `\n        <service\n            android:name=".vpn.Cv4aVpnService"\n            android:permission="android.permission.BIND_VPN_SERVICE"\n            android:exported="false"\n            android:foregroundServiceType="specialUse">\n            <intent-filter>\n                <action android:name="android.net.VpnService" />\n            </intent-filter>\n            <property\n                android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"\n                android:value="User-requested VPN tunnel for Clash Verge for Android" />\n        </service>\n`
    text = text.replace('\n        <provider\n', `${service}\n        <provider\n`)
  }
  if (!text.includes('android:name=".vpn.Cv4aVpnBootReceiver"')) {
    const receiver = `\n        <receiver\n            android:name=".vpn.Cv4aVpnBootReceiver"\n            android:enabled="true"\n            android:exported="true">\n            <intent-filter>\n                <action android:name="android.intent.action.BOOT_COMPLETED" />\n                <action android:name="android.intent.action.MY_PACKAGE_REPLACED" />\n            </intent-filter>\n        </receiver>\n`
    text = text.replace('\n        <provider\n', `${receiver}\n        <provider\n`)
  }
  if (!text.includes('android:name=".vpn.Cv4aVpnTileService"')) {
    const tiles = `\n        <service\n            android:name=".vpn.Cv4aVpnTileService"\n            android:label="@string/qs_system_proxy"\n            android:icon="@drawable/ic_qs_system_proxy"\n            android:permission="android.permission.BIND_QUICK_SETTINGS_TILE"\n            android:enabled="true"\n            android:exported="true">\n            <intent-filter>\n                <action android:name="android.service.quicksettings.action.QS_TILE" />\n            </intent-filter>\n            <meta-data\n                android:name="android.service.quicksettings.TOGGLEABLE_TILE"\n                android:value="true" />\n        </service>\n\n        <service\n            android:name=".vpn.Cv4aTunTileService"\n            android:label="@string/qs_tun_mode"\n            android:icon="@drawable/ic_qs_tun"\n            android:permission="android.permission.BIND_QUICK_SETTINGS_TILE"\n            android:enabled="true"\n            android:exported="true">\n            <intent-filter>\n                <action android:name="android.service.quicksettings.action.QS_TILE" />\n            </intent-filter>\n            <meta-data\n                android:name="android.service.quicksettings.TOGGLEABLE_TILE"\n                android:value="true" />\n        </service>\n`
    text = text.replace('\n        <provider\n', `${tiles}\n        <provider\n`)
  }
  {
    const serviceName = '.vpn.Cv4aTunTileService'
    const serviceStart = text.indexOf(`android:name="${serviceName}"`)
    if (serviceStart >= 0) {
      const serviceEnd = text.indexOf('</service>', serviceStart)
      if (serviceEnd >= 0) {
        const serviceBlock = text.slice(serviceStart, serviceEnd)
        const disabledBlock = serviceBlock.replace(
          'android:enabled="true"',
          'android:enabled="false"',
        )
        text = `${text.slice(0, serviceStart)}${disabledBlock}${text.slice(serviceEnd)}`
      }
    }
  }
  for (const serviceName of [
    '.vpn.Cv4aVpnTileService',
    '.vpn.Cv4aTunTileService',
  ]) {
    const serviceStart = text.indexOf(`android:name="${serviceName}"`)
    if (serviceStart < 0) continue
    const serviceEnd = text.indexOf('</service>', serviceStart)
    if (serviceEnd < 0) continue
    const serviceBlock = text.slice(serviceStart, serviceEnd)
    if (serviceBlock.includes('android.service.quicksettings.ACTIVE_TILE')) continue
    const toggleMeta = `            <meta-data\n                android:name="android.service.quicksettings.TOGGLEABLE_TILE"\n                android:value="true" />`
    const activeMeta = `${toggleMeta}\n            <meta-data\n                android:name="android.service.quicksettings.ACTIVE_TILE"\n                android:value="true" />`
    const blockWithActive = serviceBlock.replace(toggleMeta, activeMeta)
    if (blockWithActive === serviceBlock) {
      throw new Error(`Unable to enable active Quick Settings tile for ${serviceName}`)
    }
    text = `${text.slice(0, serviceStart)}${blockWithActive}${text.slice(serviceEnd)}`
  }
  writeFileSync(file, text)
}

function patchStrings() {
  const file = path.join(generatedApp, 'src/main/res/values/strings.xml')
  let text = readFileSync(file, 'utf8')
  for (const [name, value] of [
    ['qs_system_proxy', '系统代理'],
    ['qs_tun_mode', '虚拟网卡模式'],
  ]) {
    if (!text.includes(`name="${name}"`)) {
      text = text.replace('</resources>', `    <string name="${name}">${value}</string>\n</resources>`)
    }
  }
  writeFileSync(file, text)
}

function patchMainActivity() {
  const file = path.join(
    generatedApp,
    'src/main/java/io/github/aloofbuckle/cv4android/MainActivity.kt',
  )
  if (!existsSync(file)) return
  let text = readFileSync(file, 'utf8')
  if (!text.includes('import android.content.BroadcastReceiver')) {
    text = text.replace(
      'import android.content.res.Configuration\n',
      'import android.content.BroadcastReceiver\nimport android.content.Context\nimport android.content.Intent\nimport android.content.IntentFilter\nimport android.content.res.Configuration\n',
    )
  }
  if (!text.includes('import androidx.core.content.ContextCompat')) {
    text = text.replace(
      'import androidx.activity.enableEdgeToEdge\n',
      'import androidx.activity.enableEdgeToEdge\nimport androidx.core.content.ContextCompat\n',
    )
  }
  if (!text.includes('private val nativeStateReceiver = object : BroadcastReceiver()')) {
    text = text.replace(
      '  private var appWebView: WebView? = null\n',
      `  private var appWebView: WebView? = null\n  private val nativeStateReceiver = object : BroadcastReceiver() {\n    override fun onReceive(context: Context?, intent: Intent?) {\n      if (intent?.action == Cv4aTileSupport.ACTION_STATE_CHANGED) {\n        dispatchWebEvent("cv4a-native-state")\n      }\n    }\n  }\n`,
    )
  }
  if (!text.includes('ContextCompat.registerReceiver(')) {
    text = text.replace(
      '    super.onCreate(savedInstanceState)\n',
      `    super.onCreate(savedInstanceState)\n    ContextCompat.registerReceiver(\n      this,\n      nativeStateReceiver,\n      IntentFilter(Cv4aTileSupport.ACTION_STATE_CHANGED),\n      ContextCompat.RECEIVER_NOT_EXPORTED,\n    )\n`,
    )
  }
  text = text.replace(
    `      Cv4aTileSupport.notifyStateChanged(this)\n      appWebView?.post {\n        appWebView?.evaluateJavascript(\n          "window.dispatchEvent(new Event('cv4a-native-focus'))",\n          null,\n        )\n      }`,
    `      Cv4aTileSupport.requestTileUpdates(this)\n      dispatchWebEvent("cv4a-native-focus")`,
  )
  if (!text.includes('unregisterReceiver(nativeStateReceiver)')) {
    text = text.replace(
      '  override fun onDestroy() {\n',
      '  override fun onDestroy() {\n    unregisterReceiver(nativeStateReceiver)\n',
    )
  }
  if (!text.includes('private fun dispatchWebEvent(name: String)')) {
    text = text.replace(
      '  private fun updateSystemBars() {\n',
      `  private fun dispatchWebEvent(name: String) {\n    appWebView?.post {\n      appWebView?.evaluateJavascript(\n        "window.dispatchEvent(new Event('$name'))",\n        null,\n      )\n    }\n  }\n\n  private fun updateSystemBars() {\n`,
    )
  }
  writeFileSync(file, text)
}

export function prepareAndroidVpn({ ndk, java }) {
  if (!validCachedAar()) buildBridge({ ndk, java })
  const libs = path.join(generatedApp, 'libs')
  mkdirSync(libs, { recursive: true })
  copyFileSync(aar, path.join(libs, 'libmihomo-android.aar'))

  const sourceDir = path.resolve('packaging/android/vpn/io/github/aloofbuckle/cv4android/vpn')
  const targetDir = path.join(
    generatedApp,
    'src/main/java/io/github/aloofbuckle/cv4android/vpn',
  )
  mkdirSync(targetDir, { recursive: true })
  cpSync(sourceDir, targetDir, { recursive: true, force: true })
  cpSync(path.resolve('packaging/android/vpn/res'), path.join(generatedApp, 'src/main/res'), {
    recursive: true,
    force: true,
  })
  patchMainActivity()
  patchGradle()
  patchManifest()
  patchStrings()
}
