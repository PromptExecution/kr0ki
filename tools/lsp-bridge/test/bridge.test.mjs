import { test, after } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'
import { once } from 'node:events'
import { fileURLToPath } from 'node:url'
import { WebSocket } from 'ws'
import { createBridge, frame, parseFrames } from '../bridge.mjs'

const FAKE = fileURLToPath(new URL('./fake-server.mjs', import.meta.url))
const ORIGIN = 'http://localhost:8787'
const commands = { '/fake': { languageId: 'fake', argv: () => [process.execPath, FAKE] } }
const bridges = []
async function start(opts = {}) {
  const b = createBridge({ commands, allowedOrigins: [ORIGIN], ...opts })
  b.server.listen(0, '127.0.0.1'); await once(b.server, 'listening'); bridges.push(b)
  return { b, url: (p = '/fake') => `ws://127.0.0.1:${b.server.address().port}${p}` }
}
after(() => Promise.all(bridges.map((b) => b.close())))
const connect = (url, origin = ORIGIN) => new Promise((res, rej) => { const ws = new WebSocket(url, { origin }); ws.on('open', () => res(ws)); ws.on('error', rej); ws.on('unexpected-response', (_q, r) => rej(Object.assign(new Error(`HTTP ${r.statusCode}`), { status: r.statusCode }))) })
const next = (ws) => new Promise((res) => ws.once('message', (d) => res(d.toString())))
const alive = (pid) => { try { process.kill(pid, 0); return true } catch { return false } }
const until = async (fn, ms = 3000) => { const t = Date.now(); while (Date.now() - t < ms) { if (await fn()) return true; await new Promise((r) => setTimeout(r, 25)) } return false }

test('round-trips JSON-RPC through the stdio server and adds/strips the framing', async () => {
  const { url } = await start(); const ws = await connect(url())
  ws.send(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }))
  const reply = JSON.parse(await next(ws))
  assert.equal(reply.id, 1); assert.equal(reply.result.serverInfo.name, 'fake')
  ws.send(JSON.stringify({ jsonrpc: '2.0', method: 'textDocument/didOpen', params: { textDocument: { uri: 'file:///x.json' } } }))
  assert.equal(JSON.parse(await next(ws)).params.diagnostics[0].message, 'héllo ✓') // multi-byte text survives the byte-length framing
  ws.close()
})

test('refuses a missing or foreign Origin and unknown paths, before spawning anything', async () => {
  const { b, url } = await start()
  await assert.rejects(() => connect(url(), 'http://evil.example'), { status: 403 })
  await assert.rejects(() => new Promise((res, rej) => { const ws = new WebSocket(url()); ws.on('open', res); ws.on('error', rej); ws.on('unexpected-response', (_q, r) => rej({ status: r.statusCode })) }), { status: 403 })
  await assert.rejects(() => connect(url('/nope')), { status: 404 })
  assert.equal(b.sessions(), 0)
})

test('the browser cannot choose the command: only the allowlisted path runs, in an empty temp dir with a minimal environment', async () => {
  const { url } = await start(); const ws = await connect(url('/fake?cmd=rm%20-rf%20/&argv=--evil'))
  ws.send(JSON.stringify({ jsonrpc: '2.0', id: 7, method: 'initialize', params: { command: 'rm', args: ['-rf', '/'] } }))
  const info = JSON.parse(await next(ws)).result.serverInfo
  assert.match(info.cwd, /kr0ki-lsp-/)
  assert.deepEqual(info.envKeys.filter((k) => !['PATH', 'HOME', 'PWD', 'OLDPWD', 'SHLVL', '_'].includes(k)), [])
  ws.close()
})

test('kills the server and removes its directory when the socket closes', async () => {
  const { b, url } = await start(); const ws = await connect(url())
  ws.send(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }))
  const { pid, cwd } = JSON.parse(await next(ws)).result.serverInfo
  assert.ok(alive(pid)); assert.ok(existsSync(cwd))
  ws.close()
  assert.ok(await until(() => !alive(pid)), 'child still running')
  assert.ok(await until(() => !existsSync(cwd)), 'temp dir not removed')
  assert.ok(await until(() => b.sessions() === 0))
})

test('caps concurrent sessions', async () => {
  const { url } = await start({ maxSessions: 2 })
  const a = await connect(url()), c = await connect(url())
  await assert.rejects(() => connect(url()), { status: 503 })
  a.close(); c.close()
})

test('closes a socket whose message exceeds the limit, and one that sends binary', async () => {
  const { url } = await start({ maxMessageBytes: 2048 }); const ws = await connect(url())
  const closed = once(ws, 'close'); ws.send('x'.repeat(5000))
  const [code] = await closed; assert.equal(code, 1009)
  const ws2 = await connect(url()); const c2 = once(ws2, 'close'); ws2.send(Buffer.from([1, 2, 3]))
  assert.equal((await c2)[0], 1003)
})

test('a server that sends a hostile oversized frame is cut off, not buffered', async () => {
  const { b, url } = await start({ maxMessageBytes: 4096 }); const ws = await connect(url())
  const closed = once(ws, 'close'); ws.send(JSON.stringify({ jsonrpc: '2.0', method: 'x/bomb' }))
  assert.equal((await closed)[0], 1011)
  assert.ok(await until(() => b.sessions() === 0))
})

test('idle sessions are closed', async () => {
  const { url } = await start({ idleMs: 150 }); const ws = await connect(url())
  assert.equal((await once(ws, 'close'))[0], 1001)
})

test('parseFrames handles split chunks, several messages, and hostile headers', () => {
  const a = frame('{"a":1}'), b = frame('{"b":"é"}'); const all = Buffer.concat([a, b])
  const part = parseFrames(all.subarray(0, a.length + 5), 1024); assert.deepEqual(part.messages, ['{"a":1}'])
  const rest = parseFrames(Buffer.concat([part.rest, all.subarray(a.length + 5)]), 1024); assert.deepEqual(rest.messages, ['{"b":"é"}'])
  assert.throws(() => parseFrames(Buffer.from('Content-Length: 99999\r\n\r\n'), 1024), /exceeds/)
  assert.throws(() => parseFrames(Buffer.from('Nope: 1\r\n\r\n'), 1024), /missing/)
  assert.throws(() => parseFrames(Buffer.alloc(5000, 65), 1024), /header too large/)
})

test('/d2 is offered only when the operator names the binary in the environment', async () => {
  const { defaultCommands } = await import('../bridge.mjs')
  assert.equal('/d2' in defaultCommands({}), false)
  const c = defaultCommands({ KR0KI_D2_LSP_BIN: '/opt/d2ls' })
  assert.deepEqual(c['/d2'].argv(), ['/opt/d2ls'])
  assert.equal(c['/d2'].languageId, 'd2')
})
