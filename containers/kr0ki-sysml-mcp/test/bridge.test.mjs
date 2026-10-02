import { test, after } from 'node:test'
import assert from 'node:assert/strict'
import { once } from 'node:events'
import { fileURLToPath } from 'node:url'
import { existsSync } from 'node:fs'
import { createBridge, defaultCommand } from '../bridge.mjs'

const FAKE = fileURLToPath(new URL('./fake-mcp.mjs', import.meta.url))
const command = [process.execPath, FAKE]
const open = []
async function start(opts = {}) {
  const b = createBridge({ command, ...opts })
  b.server.listen(0, '127.0.0.1'); await once(b.server, 'listening'); open.push(b)
  const base = `http://127.0.0.1:${b.server.address().port}/mcp`
  const post = (body, sid, raw) => fetch(base, { method: 'POST', headers: { 'content-type': 'application/json', ...(sid ? { 'mcp-session-id': sid } : {}) }, body: raw ?? JSON.stringify(body) })
  const init = async () => { const r = await post({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }); return { r, sid: r.headers.get('mcp-session-id'), body: await r.json() } }
  return { b, base, post, init }
}
after(() => Promise.all(open.map((b) => b.close())))
const alive = (pid) => { try { process.kill(pid, 0); return true } catch { return false } }
const until = async (fn, ms = 3000) => { const t = Date.now(); while (Date.now() - t < ms) { if (await fn()) return true; await new Promise((r) => setTimeout(r, 25)) } return false }

test('initialize starts a session; calls with its id round-trip; stray non-JSON output from the server is ignored', async () => {
  const { post, init } = await start()
  const { r, sid, body } = await init()
  assert.equal(r.status, 200); assert.match(sid, /^[0-9a-f-]{36}$/); assert.equal(body.result.serverInfo.name, 'fake')
  assert.equal((await post({ jsonrpc: '2.0', method: 'notifications/initialized' }, sid)).status, 202)
  const list = await (await post({ jsonrpc: '2.0', id: 2, method: 'tools/list' }, sid)).json()
  assert.deepEqual(list.result.tools, [{ name: 'validate' }])
})

test('the server, not the client, chooses the session id', async () => {
  const { post } = await start()
  const r = await post({ jsonrpc: '2.0', id: 1, method: 'initialize' }, 'client-chosen-id')
  assert.equal(r.status, 200)
  const sid = r.headers.get('mcp-session-id')
  assert.notEqual(sid, 'client-chosen-id'); assert.match(sid, /^[0-9a-f-]{36}$/)
  assert.equal((await post({ jsonrpc: '2.0', id: 2, method: 'tools/list' }, 'client-chosen-id')).status, 404)
})

test('the server runs with a minimal environment', async () => {
  const { init } = await start()
  const { body } = await init()
  assert.deepEqual(body.result.serverInfo.envKeys.filter((k) => !['PATH', 'PWD', 'OLDPWD', 'SHLVL', '_'].includes(k)), [])
})

test('requests without a session are refused, and only initialize may start one', async () => {
  const { post, b } = await start()
  assert.equal((await post({ jsonrpc: '2.0', id: 1, method: 'tools/list' })).status, 404)
  assert.equal((await post({ jsonrpc: '2.0', id: 1, method: 'tools/list' }, 'not-a-session')).status, 404)
  assert.equal(b.sessions(), 0)
})

test('rejects malformed input: bad JSON, batches, non-requests, oversized bodies, other paths and methods', async () => {
  const { post, base, init } = await start({ maxBody: 2048 })
  const { sid } = await init()
  assert.equal((await post(null, sid, '{not json')).status, 400)
  assert.equal((await post([{ jsonrpc: '2.0', id: 1, method: 'x' }], sid)).status, 400) // batches refused
  assert.equal((await post({ jsonrpc: '2.0', id: 1 }, sid)).status, 400)
  assert.equal((await post(null, sid, '"just a string"')).status, 400)
  assert.equal((await post({ jsonrpc: '2.0', id: 9, method: 'x', params: { pad: 'x'.repeat(5000) } }, sid)).status, 413)
  assert.equal((await fetch(base.replace('/mcp', '/nope'))).status, 404)
  assert.equal((await fetch(base)).status, 405)
  assert.equal((await fetch(base.replace('/mcp', '/health'))).status, 200)
})

test('caps concurrent sessions and reclaims a slot when a session ends', async () => {
  const { post, base, init, b } = await start({ maxSessions: 2 })
  const a = await init(); await init()
  assert.equal((await post({ jsonrpc: '2.0', id: 1, method: 'initialize' })).status, 503)
  await fetch(base, { method: 'DELETE', headers: { 'mcp-session-id': a.sid } })
  assert.ok(await until(() => b.sessions() === 1))
  assert.equal((await post({ jsonrpc: '2.0', id: 1, method: 'initialize' })).status, 200)
})

test('DELETE kills the child process', async () => {
  const { base, init } = await start()
  const { sid, body } = await init(); const pid = body.result.serverInfo.pid
  assert.ok(alive(pid))
  assert.equal((await fetch(base, { method: 'DELETE', headers: { 'mcp-session-id': sid } })).status, 204)
  assert.ok(await until(() => !alive(pid)), 'child still running')
})

test('a child that dies frees its session and fails the in-flight call instead of hanging', async () => {
  const { post, init, b } = await start()
  const { sid } = await init()
  const r = await post({ jsonrpc: '2.0', id: 5, method: 'x/crash' }, sid)
  assert.equal(r.status, 502)
  assert.ok(await until(() => b.sessions() === 0))
})

test('a server that never answers yields 504 after the call timeout', async () => {
  const { post, init } = await start({ callMs: 150 })
  const { sid } = await init()
  assert.equal((await post({ jsonrpc: '2.0', id: 7, method: 'x/silent' }, sid)).status, 504)
})

test('the same request id twice in flight is refused rather than answered to the wrong caller', async () => {
  const { post, init } = await start()
  const { sid } = await init()
  const slow = post({ jsonrpc: '2.0', id: 3, method: 'x/slow' }, sid)
  await new Promise((r) => setTimeout(r, 50))
  assert.equal((await post({ jsonrpc: '2.0', id: 3, method: 'tools/list' }, sid)).status, 409)
  assert.equal((await (await slow).json()).result, 'slow')
})

test('idle sessions are killed', async () => {
  const { init, b } = await start({ idleMs: 150 })
  const { body } = await init(); const pid = body.result.serverInfo.pid
  assert.ok(await until(() => b.sessions() === 0 && !alive(pid)))
})

test('the default command is the real sysml-v2-lsp MCP server file, and a bridge without it fails at startup', () => {
  const [node, file] = defaultCommand() // throws if the dependency is not installed
  assert.equal(node, process.execPath)
  assert.ok(file.endsWith('mcpServer.js') && existsSync(file), file)
})
