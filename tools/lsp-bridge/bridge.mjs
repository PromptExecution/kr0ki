// WebSocket <-> stdio bridge for language servers, used by the playbook's code editor (optional, "when available").
//
// SECURITY MODEL. This process spawns local binaries on behalf of a web page, so it is deliberately narrow:
//   * binds 127.0.0.1 by default (set LSP_BRIDGE_HOST to change it; do not expose it);
//   * the browser picks a language by URL path (/json, /yaml) and NEVER supplies a command or arguments: the
//     argv for each path is a fixed allowlist in COMMANDS;
//   * the WebSocket Origin must be in the allowed list (browsers always send it); requests without one are refused;
//   * one process per socket, killed when the socket closes; at most MAX_SESSIONS at once; messages and frames are
//     size-capped; the server runs in an empty temp directory with a minimal environment; idle sessions are closed.
//
// Wire format: the editor sends and receives bare JSON-RPC strings; this bridge adds/strips the `Content-Length:`
// framing that stdio language servers speak.
import http from 'node:http'
import { spawn } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRequire } from 'node:module'
import { WebSocketServer } from 'ws'

const HERE = dirname(fileURLToPath(import.meta.url))
const require_ = createRequire(import.meta.url)

/** Resolve a package's bin script without invoking a shell. */
function binOf(pkg, bin) {
  const pj = require_(`${pkg}/package.json`)
  const rel = typeof pj.bin === 'string' ? pj.bin : pj.bin[bin]
  return join(dirname(require_.resolve(`${pkg}/package.json`)), rel)
}

/** The ONLY commands the bridge will run. Keyed by URL path; argv is fixed. */
export function defaultCommands() {
  return {
    '/json': { languageId: 'json', argv: () => [process.execPath, binOf('vscode-langservers-extracted', 'vscode-json-language-server'), '--stdio'] },
    '/yaml': { languageId: 'yaml', argv: () => [process.execPath, binOf('yaml-language-server', 'yaml-language-server'), '--stdio'] },
  }
}

export function defaultOrigins(env = process.env) {
  const base = ['http://localhost:8787', 'http://127.0.0.1:8787', 'http://localhost:5173', 'http://127.0.0.1:5173']
  const extra = (env.LSP_BRIDGE_ORIGINS || env.KR0KI_PUBLIC_URL || '').split(',').map((s) => s.trim().replace(/\/$/, '')).filter(Boolean)
  return [...new Set([...base, ...extra])]
}

/** Incremental parser for `Content-Length:`-framed messages. Returns {messages, rest}; throws on a hostile header. */
export function parseFrames(buffer, maxFrame) {
  const messages = []
  let rest = buffer
  for (;;) {
    const headerEnd = rest.indexOf('\r\n\r\n')
    if (headerEnd === -1) {
      if (rest.length > 4096) throw new Error('header too large')
      break
    }
    const header = rest.subarray(0, headerEnd).toString('ascii')
    const m = /^content-length:\s*(\d+)\s*$/im.exec(header)
    if (!m) throw new Error('missing Content-Length')
    const len = Number(m[1])
    if (len > maxFrame) throw new Error(`frame of ${len} bytes exceeds the limit`)
    if (rest.length < headerEnd + 4 + len) break
    messages.push(rest.subarray(headerEnd + 4, headerEnd + 4 + len).toString('utf8'))
    rest = rest.subarray(headerEnd + 4 + len)
  }
  return { messages, rest }
}

export const frame = (text) => Buffer.concat([Buffer.from(`Content-Length: ${Buffer.byteLength(text)}\r\n\r\n`, 'ascii'), Buffer.from(text, 'utf8')])

export function createBridge({
  commands = defaultCommands(),
  allowedOrigins = defaultOrigins(),
  maxSessions = 4,
  maxMessageBytes = 1 << 20,
  idleMs = 10 * 60 * 1000,
  log = () => {},
  trace = false, // log JSON-RPC method names (never bodies) in both directions
} = {}) {
  const server = http.createServer((req, res) => {
    res.writeHead(426, { 'Content-Type': 'text/plain' })
    res.end('WebSocket only: connect to /json or /yaml\n')
  })
  const wss = new WebSocketServer({ noServer: true, maxPayload: maxMessageBytes })
  let sessions = 0

  const reject = (socket, code, text) => {
    socket.write(`HTTP/1.1 ${code} ${text}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n`)
    socket.destroy()
  }

  server.on('upgrade', (req, socket, head) => {
    const origin = req.headers.origin
    if (!origin || !allowedOrigins.includes(origin.replace(/\/$/, ''))) {
      log('refused origin', origin)
      return reject(socket, 403, 'Forbidden')
    }
    const path = new URL(req.url, 'http://x').pathname
    const spec = Object.hasOwn(commands, path) ? commands[path] : null
    if (!spec) return reject(socket, 404, 'Not Found')
    if (sessions >= maxSessions) return reject(socket, 503, 'Service Unavailable')
    wss.handleUpgrade(req, socket, head, (ws) => run(ws, spec, path))
  })

  function traceMessage(dir, text) {
    try {
      const m = JSON.parse(text)
      const td = m.params?.textDocument
      const detail = [
        m.params?.diagnostics ? `diagnostics=${m.params.diagnostics.length}` : '',
        td ? `uri=${td.uri} lang=${td.languageId ?? '-'} v=${td.version ?? '-'} len=${td.text?.length ?? m.params?.contentChanges?.[0]?.text?.length ?? '-'}` : '',
        m.method === 'initialize' ? `caps.textDocument=${Object.keys(m.params?.capabilities?.textDocument || {}).join(',')} caps.workspace=${Object.keys(m.params?.capabilities?.workspace || {}).join(',')}` : '',
      ].filter(Boolean).join(' ')
      log(`${dir} ${m.method || `response#${m.id}${m.error ? ' ERROR' : ''}`} ${detail}`.trim())
    } catch { log(`${dir} (unparseable, ${text.length} bytes)`) }
  }

  function run(ws, spec, path) {
    sessions++
    const cwd = mkdtempSync(join(tmpdir(), 'kr0ki-lsp-'))
    const [cmd, ...args] = spec.argv()
    const child = spawn(cmd, args, { cwd, env: { PATH: process.env.PATH, HOME: cwd }, stdio: ['pipe', 'pipe', 'pipe'] })
    log('session start', path, child.pid)
    let buf = Buffer.alloc(0)
    let done = false
    let idle = setTimeout(() => finish(1001, 'idle'), idleMs)
    const touch = () => { clearTimeout(idle); idle = setTimeout(() => finish(1001, 'idle'), idleMs) }
    function finish(code, reason) {
      if (done) return
      done = true
      clearTimeout(idle)
      sessions--
      try { child.kill('SIGKILL') } catch { /* already gone */ }
      try { rmSync(cwd, { recursive: true, force: true }) } catch { /* best effort */ }
      try { ws.close(code, reason) } catch { /* already closed */ }
      log('session end', path, child.pid, reason)
    }
    child.stdout.on('data', (chunk) => {
      touch()
      buf = Buffer.concat([buf, chunk])
      try {
        const { messages, rest } = parseFrames(buf, maxMessageBytes)
        buf = rest
        for (const m of messages) {
          if (trace) traceMessage('<-', m)
          if (ws.readyState === ws.OPEN) ws.send(m)
        }
      } catch (err) {
        finish(1011, String(err.message).slice(0, 100))
      }
    })
    child.stderr.on('data', () => {}) // language servers log here; never forward it to the page
    child.on('exit', () => finish(1011, 'server exited'))
    child.on('error', () => finish(1011, 'could not start server'))
    ws.on('message', (data, isBinary) => {
      if (isBinary) return finish(1003, 'text only')
      touch()
      const text = data.toString('utf8')
      if (trace) traceMessage('->', text)
      if (child.stdin.writable) child.stdin.write(frame(text))
    })
    ws.on('close', () => finish(1000, 'closed'))
    ws.on('error', () => finish(1011, 'socket error'))
  }

  return { server, sessions: () => sessions, close: () => new Promise((r) => { wss.clients.forEach((c) => c.terminate()); server.close(() => r()) }) }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const host = process.env.LSP_BRIDGE_HOST || '127.0.0.1'
  const port = Number(process.env.LSP_BRIDGE_PORT || 8791)
  const bridge = createBridge({ log: (...a) => console.error('[lsp-bridge]', ...a), trace: process.env.LSP_BRIDGE_TRACE === '1' })
  bridge.server.listen(port, host, () => console.error(`[lsp-bridge] ws://${host}:${port}/json  /yaml   origins: ${defaultOrigins().join(', ')}`))
}
