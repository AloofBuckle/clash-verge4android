import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { prepareAndroidRuntime } from './android-runtime.mjs'
import { prepareAndroidVpn } from './android-vpn.mjs'

const action = process.argv[2]
if (!['init', 'build', 'dev'].includes(action))
  throw new Error('Use android:init, android:build or android:dev')
const sdk =
  process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT || '/opt/android-sdk'
if (!existsSync(sdk))
  throw new Error('Set ANDROID_HOME to the Android SDK directory')
const ndks = existsSync(path.join(sdk, 'ndk'))
  ? readdirSync(path.join(sdk, 'ndk')).sort((a, b) =>
      b.localeCompare(a, undefined, { numeric: true }),
    )
  : []
const ndk =
  process.env.NDK_HOME || (ndks[0] ? path.join(sdk, 'ndk', ndks[0]) : '')
if (!ndk) throw new Error('Install an Android NDK or set NDK_HOME')
const localJdk = path.resolve('.local-artifacts/toolchain/jdk21')
const java =
  process.env.JAVA_HOME || (existsSync(localJdk) ? localJdk : undefined)
const extra = process.argv.slice(3).filter((arg) => arg !== '--')
if (action !== 'init') {
  prepareAndroidRuntime({ ndk })
  prepareAndroidVpn({ ndk, java })
}
const debug = extra.includes('--debug') || extra.includes('-d')
const forwarded = extra.filter((arg) => arg !== '--debug' && arg !== '-d')
const release = action === 'build' && !debug
const env = {
  ...process.env,
  ANDROID_HOME: sdk,
  ANDROID_SDK_ROOT: sdk,
  NDK_HOME: ndk,
  ...(java ? { JAVA_HOME: java, PATH: `${java}/bin:${process.env.PATH}` } : {}),
  ...(debug
    ? {
        CARGO_PROFILE_DEV_DEBUG: process.env.CARGO_PROFILE_DEV_DEBUG || '0',
        CARGO_PROFILE_DEV_STRIP:
          process.env.CARGO_PROFILE_DEV_STRIP || 'symbols',
      }
    : {}),
  ...(release
    ? {
        CARGO_PROFILE_RELEASE_DEBUG:
          process.env.CARGO_PROFILE_RELEASE_DEBUG || '0',
        CARGO_PROFILE_RELEASE_STRIP:
          process.env.CARGO_PROFILE_RELEASE_STRIP || 'symbols',
      }
    : {}),
}
const hasTarget = forwarded.some(
  (arg) => arg === '--target' || arg === '-t' || arg.startsWith('--target='),
)
const options =
  action === 'build'
    ? [
        '--apk',
        ...(debug ? ['--debug'] : []),
        ...(hasTarget ? [] : ['--target', 'aarch64']),
      ]
    : []
const result = spawnSync(
  'pnpm',
  ['exec', 'tauri', 'android', action, ...options, ...forwarded],
  { stdio: 'inherit', env },
)
process.exit(result.status ?? 1)
