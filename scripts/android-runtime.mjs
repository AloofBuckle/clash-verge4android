import { createHash } from 'node:crypto'
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
} from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'

const version = 'v1.19.31'
const archiveName = `mihomo-android-arm64-v8-${version}.gz`
const archiveSha256 =
  'de00bc53ed151636ca078c812a82a5315687d8d52164db230f1935b2a37904f6'
const geositeSha256 =
  '12dfa24f466986cfe1ead0c67e554e5c67642bb6b204ea64172178d25825f216'
const runtimeDir = path.resolve('.local-artifacts/runtime')
const archive = path.join(runtimeDir, `mihomo-android-${version}.gz`)
const geosite = path.join(runtimeDir, 'geosite.dat')
const agent = path.join(runtimeDir, 'cv4a-root-agent-arm64')

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { stdio: 'inherit', env })
  if (result.status !== 0)
    throw new Error(`${command} exited with status ${result.status}`)
}

function sha256(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex')
}

function ensureMihomo() {
  mkdirSync(runtimeDir, { recursive: true })
  if (!existsSync(archive) || sha256(archive) !== archiveSha256) {
    rmSync(archive, { force: true })
    const url = `https://github.com/MetaCubeX/mihomo/releases/download/${version}/${archiveName}`
    run('curl', [
      '-fL',
      '--retry',
      '3',
      '--connect-timeout',
      '15',
      url,
      '-o',
      archive,
    ])
  }
  if (sha256(archive) !== archiveSha256)
    throw new Error(`Mihomo ${version} checksum mismatch`)
}

function ensureGeosite() {
  mkdirSync(runtimeDir, { recursive: true })
  if (!existsSync(geosite) || sha256(geosite) !== geositeSha256) {
    rmSync(geosite, { force: true })
    run('curl', [
      '-fL',
      '--retry',
      '3',
      '--connect-timeout',
      '15',
      'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat',
      '-o',
      geosite,
    ])
  }
  if (sha256(geosite) !== geositeSha256)
    throw new Error('GeoSite.dat checksum mismatch')
}

function buildAgent(ndk) {
  const toolchain = path.join(
    ndk,
    'toolchains/llvm/prebuilt/linux-x86_64/bin',
  )
  const linker = path.join(toolchain, 'aarch64-linux-android29-clang')
  const cxx = path.join(toolchain, 'aarch64-linux-android29-clang++')
  const ar = path.join(toolchain, 'llvm-ar')
  if (!existsSync(linker)) throw new Error(`Android linker not found: ${linker}`)
  if (!existsSync(cxx)) throw new Error(`Android C++ compiler not found: ${cxx}`)
  if (!existsSync(ar)) throw new Error(`Android archiver not found: ${ar}`)
  run('cargo', ['build', '-p', 'cv4a-root-agent', '--target', 'aarch64-linux-android', '--release'], {
    ...process.env,
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: linker,
    CARGO_TARGET_AARCH64_LINUX_ANDROID_AR: ar,
    CC_aarch64_linux_android: linker,
    CXX_aarch64_linux_android: cxx,
    AR_aarch64_linux_android: ar,
  })
  copyFileSync(
    path.resolve('target/aarch64-linux-android/release/cv4a-root-agent'),
    agent,
  )
}

export function prepareAndroidRuntime({ ndk }) {
  ensureMihomo()
  ensureGeosite()
  buildAgent(ndk)
}

