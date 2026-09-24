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
  patchGradle()
  patchManifest()
}
