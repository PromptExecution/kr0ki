// HTTP front for a stdio MCP server (sysml-v2-lsp's `sysml-mcp`), run as a kr0ki sidecar.
//
//   POST /mcp     one JSON-RPC message per request. `initialize` starts a session (a fixed child process) and returns
//                 `Mcp-Session-Id`; later calls must carry it. Notifications get 202; requests get the child's response.
//   DELETE /mcp   ends the session.     GET /health   liveness.
//
// Security: the command is FIXED (never taken from a request); one child per session with a minimal environment; body, session,
// idle and per-call limits; JSON-RPC batches are refused. It has NO authentication: bind it to loopback (the default) or keep it
// inside the pod network namespace, and let kr0ki-server (which has bearer auth) be the only client. Stdio MCP here is newline-delimited JSON.
import http from 'node:http'
import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

// The package restricts its `exports` to the main entry, so its MCP server file cannot be resolved by subpath: locate it in
// node_modules (npm and pnpm both link the package there) and fail loudly if it is not installed.
export function defaultCommand() {
  const file = join(dirname(fileURLToPath(import.meta.url)), 'node_modules', 'sysml-v2-lsp', 'dist', 'server', 'mcpServer.js')
  if (!existsSync(file)) throw new Error(`sysml-v2-lsp is not installed (expected ${file}); run npm install`)
  return [process.execPath, file]
}

export function createBridge({ command = null, maxBody = 1 << 20, maxSessions = 8, idleMs = 5 * 60_000, callMs = 30_000, log = () => {} } = {}) {
  command = command ?? defaultCommand() // resolved here so a missing dependency fails at startup, not in the first request
  const sessions = new Map()

  function end(id, why) {
    const s = sessions.get(id)
    if (!s) return
    sessions.delete(id)
    clearTimeout(s.idle)
    for (const [, p] of s.pending) { clearTimeout(p.timer); p.fail(502, `server ${why}`) }
    s.pending.clear()
    try { s.child.kill('SIGKILL') } catch { /* already gone */ }
    log('session end', id, why)
  }

  function start() {
    const child = spawn(command[0], command.slice(1), { stdio: ['pipe', 'pipe', 'ignore'], env: { PATH: process.env.PATH } })
    const s = { child, buf: '', pending: new Map(), idle: null }
    child.stdout.on('data', (chunk) => {
      s.buf += chunk
      for (let i; (i = s.buf.indexOf('\n')) >= 0; ) {
        const line = s.buf.slice(0, i).trim()
        s.buf = s.buf.slice(i + 1)
        if (!line) continue
        let msg
        try { msg = JSON.parse(line) } catch { continue } // not JSON-RPC (a stray log line)
        const p = msg.id !== undefined && msg.id !== null ? s.pending.get(msg.id) : null
        if (p) { s.pending.delete(msg.id); clearTimeout(p.timer); p.ok(line) }
      }
      if (s.buf.length > maxBody) end(s.id, 'sent an oversized line')
    })
    child.on('exit', () => end(s.id, 'exited'))
    child.on('error', () => end(s.id, 'could not start'))
    return s
  }

  const touch = (s) => { clearTimeout(s.idle); s.idle = setTimeout(() => end(s.id, 'idle'), idleMs) }
  const reply = (res, code, body = '', headers = {}) => { res.writeHead(code, { 'content-type': 'application/json', ...headers }); res.end(body) }
  const error = (res, code, message, headers) => reply(res, code, JSON.stringify({ error: message }), headers)

  const server = http.createServer((req, res) => {
    const path = new URL(req.url, 'http://x').pathname
    if (path === '/health') return reply(res, 200, '{"ok":true}')
    if (path !== '/mcp') return error(res, 404, 'not found')
    const sid = req.headers['mcp-session-id']
    if (req.method === 'DELETE') { if (sid) end(sid, 'deleted'); return reply(res, 204) }
    if (req.method !== 'POST') return error(res, 405, 'POST only')

    let size = 0
    const chunks = []
    let tooBig = false
    req.on('data', (c) => { size += c.length; if (size > maxBody) tooBig = true; else chunks.push(c) })
    req.on('end', () => {
      if (tooBig) return error(res, 413, 'request too large')
      let msg
      try { msg = JSON.parse(Buffer.concat(chunks).toString('utf8')) } catch { return error(res, 400, 'invalid JSON') }
      if (!msg || Array.isArray(msg) || typeof msg !== 'object' || typeof msg.method !== 'string') return error(res, 400, 'send exactly one JSON-RPC request or notification')

      let id = typeof sid === 'string' ? sid : undefined
      let s = id ? sessions.get(id) : undefined
      const headers = {}
      if (!s) {
        if (msg.method !== 'initialize') return error(res, 404, 'no such session: send initialize first')
        if (sessions.size >= maxSessions) return error(res, 503, 'too many sessions')
        s = start(); id = randomUUID(); s.id = id; sessions.set(id, s); headers['mcp-session-id'] = id
        log('session start', id)
      }
      touch(s)
      const line = JSON.stringify(msg) + '\n'
      if (msg.id === undefined || msg.id === null) { s.child.stdin.write(line); return reply(res, 202, '', headers) }
      if (s.pending.has(msg.id)) return error(res, 409, 'a request with this id is already in flight', headers)
      const timer = setTimeout(() => { s.pending.delete(msg.id); error(res, 504, 'the MCP server did not answer in time', headers) }, callMs)
      s.pending.set(msg.id, { timer, ok: (body) => reply(res, 200, body, headers), fail: (code, why) => error(res, code, why, headers) })
      s.child.stdin.write(line)
    })
  })

  return { server, sessions: () => sessions.size, close: () => new Promise((r) => { for (const id of [...sessions.keys()]) end(id, 'shutdown'); server.close(() => r()) }) }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const host = process.env.BIND_HOST || '127.0.0.1'
  const port = Number(process.env.PORT || 8790)
  createBridge({ log: (...a) => console.error('[sysml-mcp]', ...a) }).server.listen(port, host, () => console.error(`[sysml-mcp] http://${host}:${port}/mcp`))
}
