import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import http from 'node:http'
import { after, before, test } from 'node:test'

const endpoint = process.env.CV4A_CDP_URL
if (!endpoint)
  throw new Error('Set CV4A_CDP_URL to the forwarded debug WebView endpoint')
const artifactDir = '.local-artifacts/mobile-e2e'
const prefix = `E2E-${Date.now()}`
const yaml = `proxies:
- {name: one, type: hysteria2, server: example.test, port: 443, password: test}
- {name: two, type: hysteria2, server: example.test, port: 443, password: test}
proxy-groups:
- {name: '1', type: select, proxies: [one, two]}
- {name: '2', type: select, proxies: [two, one]}
rules:
- GEOSITE,category-dev,1
- MATCH,2
x-preserve: {nested: [original, data]}
`
const hash = (value) => createHash('sha256').update(value).digest('hex')
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let ws,
  nextId = 0,
  server,
  fixtureUrl,
  fixtureBody = yaml,
  fixtureStatus = 200
const pending = new Map()

async function connect() {
  const targets = await (await fetch(`${endpoint}/json/list`)).json()
  const target = targets.find(
    (t) => t.type === 'page' && /tauri|localhost/.test(t.url),
  )
  assert.ok(target, 'Expected the running CV4A debug WebView')
  ws = new WebSocket(target.webSocketDebuggerUrl)
  ws.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const item = pending.get(message.id)
    if (!item) return
    pending.delete(message.id)
    clearTimeout(item.timer)
    if (message.error) item.reject(new Error(message.error.message))
    else item.resolve(message.result)
  })
  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true })
    ws.addEventListener('error', reject, { once: true })
  })
  await send('Runtime.enable')
  await send('Page.enable')
}
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++nextId
    const timer = setTimeout(() => {
      pending.delete(id)
      reject(new Error(`CDP timeout: ${method}`))
    }, 35000)
    pending.set(id, { resolve, reject, timer })
    ws.send(JSON.stringify({ id, method, params }))
  })
}
async function evaluate(expression) {
  const result = await send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
  })
  if (result.exceptionDetails)
    throw new Error(
      result.exceptionDetails.exception?.description ||
        result.exceptionDetails.text,
    )
  return result.result?.value
}
const api = (command, args = {}) =>
  evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})`,
  )
async function waitFor(expression) {
  for (let i = 0; i < 80; i++) {
    try {
      if (await evaluate(expression)) return
    } catch {
      /* Page may still be navigating. */
    }
    await sleep(150)
  }
  throw new Error(`UI condition did not become true: ${expression}`)
}
async function reload() {
  await send('Page.reload')
  await sleep(350)
  await waitFor(
    '!!window.__TAURI_INTERNALS__ && !!document.querySelector(".bottom-nav")',
  )
  await waitFor('!document.body.innerText.includes("正在读取本机配置")')
}
async function nav(label) {
  await evaluate(
    `Array.from(document.querySelectorAll('.bottom-nav button')).find(b => b.textContent.trim() === ${JSON.stringify(label)}).click()`,
  )
  await sleep(150)
}
async function fill(selector, value) {
  await evaluate(`(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    const prototype = element.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, 'value').set.call(element, ${JSON.stringify(value)});
    element.dispatchEvent(new Event('input', {bubbles: true}));
  })()`)
}
async function screenshot(name) {
  const { data } = await send('Page.captureScreenshot', { format: 'png' })
  await writeFile(`${artifactDir}/${name}.png`, Buffer.from(data, 'base64'))
}

before(async () => {
  await mkdir(artifactDir, { recursive: true })
  server = http.createServer((_request, response) => {
    response.writeHead(fixtureStatus, { 'Content-Type': 'text/yaml' })
    response.end(fixtureBody)
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  fixtureUrl = `http://10.0.2.2:${server.address().port}/subscription.yaml`
  await connect()
  await reload()
})
after(async () => {
  ws?.close()
  for (const item of pending.values()) clearTimeout(item.timer)
  server?.closeAllConnections()
  await new Promise((resolve) => server?.close(resolve))
})

test('real Tauri backend reports unavailable runtime capabilities', async () => {
  const capabilities = await api('mobile_capabilities')
  assert.equal(capabilities.stage, 'android-bootstrap')
  assert.equal(capabilities.profileManagement, true)
  for (const key of ['tunControl', 'systemProxy', 'liveCore'])
    assert.equal(capabilities[key], false)
  assert.equal(
    await evaluate('document.querySelector(".power-button").disabled'),
    true,
  )
  await screenshot('home')
})

test('native storage preserves complete YAML, groups and rule order', async () => {
  const document = await api('mobile_import', { name: `${prefix}-local`, yaml })
  const profile = document.profiles.at(-1)
  assert.equal(profile.yaml, yaml)
  await api('mobile_activate', { id: profile.id })
  const summary = await api('mobile_inspect', { yaml: profile.yaml })
  assert.deepEqual(
    summary.groups.map((g) => g.members),
    [
      ['one', 'two'],
      ['two', 'one'],
    ],
  )
  assert.deepEqual(summary.rules, ['GEOSITE,category-dev,1', 'MATCH,2'])
  const before = await api('mobile_profiles')
  await assert.rejects(
    api('mobile_import', { name: 'bad', yaml: '<html>error</html>' }),
  )
  await assert.rejects(
    api('mobile_import', { name: 'bad', yaml: 'proxies: [' }),
  )
  assert.deepEqual(await api('mobile_profiles'), before)
})

test('URI import preserves every node and rejects unsupported options', async () => {
  const document = await api('mobile_import_nodes', {
    name: `${prefix}-nodes`,
    links:
      'vless://11111111-1111-1111-1111-111111111111@example.test:443?security=tls&sni=tls.example.test#TLS\nhysteria2://p%40ss@example.test:443?sni=hy.example.test#HY2',
  })
  const imported = document.profiles.at(-1)
  const summary = await api('mobile_inspect', { yaml: imported.yaml })
  assert.deepEqual(
    summary.nodes.map((n) => n.name),
    ['TLS', 'HY2'],
  )
  assert.match(imported.yaml, /servername: tls\.example\.test/)
  await assert.rejects(
    api('mobile_import_nodes', {
      name: 'must-not-save',
      links: 'hy2://password@example.test:443?unsupported=1',
    }),
  )
  assert.equal(
    (await api('mobile_profiles')).profiles.length,
    document.profiles.length,
  )
})

test('subscription refresh replaces atomically and retains data on errors', {
  timeout: 90000,
}, async () => {
  const document = await api('mobile_subscribe', {
    name: `${prefix}-remote`,
    url: fixtureUrl,
  })
  const id = document.profiles.at(-1).id
  fixtureBody = yaml.replace('x-preserve:', 'x-updated:')
  let refreshed = await api('mobile_refresh', { id })
  assert.equal(refreshed.profiles.find((p) => p.id === id).yaml, fixtureBody)
  const saved = refreshed.profiles.find((p) => p.id === id).yaml
  fixtureStatus = 503
  await assert.rejects(api('mobile_refresh', { id }))
  fixtureStatus = 200
  fixtureBody = '<html>unexpected upstream page</html>'
  await assert.rejects(api('mobile_refresh', { id }))
  fixtureBody = 'x'.repeat(4 * 1024 * 1024 + 1)
  await assert.rejects(api('mobile_refresh', { id }))
  refreshed = await api('mobile_profiles')
  assert.equal(refreshed.profiles.find((p) => p.id === id).yaml, saved)
  fixtureBody = yaml
})

test('HTTPS subscription reaches the real server without losing nodes', {
  skip: !process.env.CV4A_SUB_URL,
  timeout: 60000,
}, async () => {
  const url = process.env.CV4A_SUB_URL
  const original = await (await fetch(url)).text()
  const document = await api('mobile_subscribe', {
    name: `${prefix}-HTTPS`,
    url,
  })
  const profile = document.profiles.at(-1)
  assert.equal(
    hash(profile.yaml),
    hash(original),
    'Downloaded YAML must be byte-for-byte identical',
  )
  const expected = await api('mobile_inspect', { yaml: original })
  const actual = await api('mobile_inspect', { yaml: profile.yaml })
  assert.deepEqual(actual, expected)
})

test('portrait GUI imports a profile through the real Rust command', async () => {
  await reload()
  await nav('配置')
  await evaluate(
    `Array.from(document.querySelectorAll('.import-actions button')).find(b => b.textContent.includes('导入 YAML')).click()`,
  )
  await waitFor('document.querySelector("dialog").open')
  await fill('dialog input[name="name"]', `${prefix}-GUI`)
  await fill('dialog textarea', yaml)
  await evaluate('document.querySelector("dialog form").requestSubmit()')
  await waitFor('!document.querySelector("dialog").open')
  await evaluate(
    `Array.from(document.querySelectorAll('.profile-card')).find(c => c.textContent.includes(${JSON.stringify(`${prefix}-GUI`)})).querySelector('.profile-actions button').click()`,
  )
  await waitFor(
    `document.querySelector('.profile-card.selected')?.textContent.includes(${JSON.stringify(`${prefix}-GUI`)})`,
  )
  await screenshot('profiles')
  await nav('代理')
  await waitFor('document.querySelectorAll(".group-card").length === 2')
  assert.equal(
    await evaluate(
      'Array.from(document.querySelectorAll(".node,.mode-row button")).every(b => b.disabled)',
    ),
    true,
  )
  assert.equal(await evaluate('document.querySelectorAll(".node").length'), 4)
  assert.equal(
    await evaluate('document.documentElement.scrollWidth <= innerWidth'),
    true,
  )
  await screenshot('proxies')
  await nav('规则')
  await fill('input.search', 'category-dev')
  await waitFor('document.querySelectorAll(".rule").length === 1')
  await screenshot('rules')
  await nav('设置')
  assert.equal(
    await evaluate(
      'Array.from(document.querySelectorAll("[role=switch]")).every(b => b.disabled && !b.checked)',
    ),
    true,
  )
  await screenshot('settings')
})

test('Android document picker can select and import a YAML file', {
  skip: !process.env.CV4A_TEST_DEVICE,
  timeout: 60000,
}, async () => {
  const serial = process.env.CV4A_TEST_DEVICE
  assert.match(serial, /^emulator-/)
  const adb = (...args) =>
    execFileSync('adb', ['-s', serial, ...args], {
      timeout: 15000,
      stdio: 'pipe',
    }).toString()
  const file = 'cv4a-native-picker.yaml'
  await writeFile(`${artifactDir}/${file}`, yaml)
  adb('push', `${artifactDir}/${file}`, `/sdcard/Download/${file}`)
  await reload()
  await nav('配置')
  await evaluate(
    `Array.from(document.querySelectorAll('.import-actions button')).find(b => b.textContent.includes('YAML')).click()`,
  )
  await waitFor('document.querySelector("dialog").open')
  await evaluate('document.querySelector("input[type=file]").click()')
  await sleep(500)
  const nodes = () => {
    adb('shell', 'uiautomator', 'dump', '/sdcard/cv4a-test-window.xml')
    return (
      adb('exec-out', 'cat', '/sdcard/cv4a-test-window.xml').match(
        /<node\b[^>]*>/g,
      ) || []
    )
  }
  const tap = (node) => {
    assert.ok(node, 'Document picker item must exist')
    assert.match(node, /enabled="true"/, 'Document must be selectable')
    const [, x1, y1, x2, y2] = node.match(
      /bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/,
    )
    adb(
      'shell',
      'input',
      'tap',
      String(Math.round((+x1 + +x2) / 2)),
      String(Math.round((+y1 + +y2) / 2)),
    )
  }
  let entries = nodes()
  if (!entries.some((n) => n.includes(`text="${file}"`))) {
    tap(entries.find((n) => n.includes('content-desc="Show roots"')))
    tap(nodes().find((n) => n.includes('text="Downloads"')))
    entries = nodes()
  }
  tap(entries.find((n) => n.includes(`text="${file}"`)))
  await waitFor(
    'document.querySelector("dialog textarea")?.value.includes("x-preserve")',
  )
  await evaluate('document.querySelector("dialog form").requestSubmit()')
  await waitFor('!document.querySelector("dialog").open')
  const profile = (await api('mobile_profiles')).profiles.at(-1)
  assert.equal(profile.name, file.replace('.yaml', ''))
  assert.equal(profile.yaml, yaml)
})

test('profile selection survives a complete Android process restart', {
  skip: !process.env.CV4A_TEST_DEVICE,
  timeout: 60000,
}, async () => {
  const serial = process.env.CV4A_TEST_DEVICE
  assert.match(
    serial,
    /^emulator-/,
    'This test only restarts the dedicated test emulator app',
  )
  const beforeRestart = hash(JSON.stringify(await api('mobile_profiles')))
  ws.close()
  const adb = (...args) =>
    execFileSync('adb', ['-s', serial, ...args], {
      timeout: 15000,
      stdio: 'pipe',
    })
  adb('shell', 'am', 'force-stop', 'io.github.aloofbuckle.cv4android')
  adb(
    'shell',
    'am',
    'start',
    '-n',
    'io.github.aloofbuckle.cv4android/.MainActivity',
  )
  await sleep(2000)
  const pid = adb('shell', 'pidof', 'io.github.aloofbuckle.cv4android')
    .toString()
    .trim()
  const port = new URL(endpoint).port
  adb('forward', `tcp:${port}`, `localabstract:webview_devtools_remote_${pid}`)
  await connect()
  await waitFor('!!window.__TAURI_INTERNALS__')
  assert.equal(
    hash(JSON.stringify(await api('mobile_profiles'))),
    beforeRestart,
  )
})
