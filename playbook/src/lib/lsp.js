// Language intelligence for the source editor: syntax highlighting where a CodeMirror language exists, and an
// optional Language Server (LSP) connection per diagram language, "when available".
//
// Honest scope (verified 2026-10-01): of kr0ki's 26 diagram languages only JSON (vega, vegalite), YAML (wireviz) and, via a built Go binary, D2 have a
// language server we can run. wavedrom is JSON5-ish, so a strict JSON server would flag valid sources; it gets none.
// Everything else edits as plain text. A server is used only when the user configures a WebSocket URL for that language
// (Setup -> Language servers) and the connection succeeds; any failure degrades silently to the plain editor.
import { json } from '@codemirror/lang-json'
import { yaml } from '@codemirror/lang-yaml'
import { LSPClient, languageServerExtensions } from '@codemirror/lsp-client'

/** format -> { highlight, lsp: languageId | null }. `lsp` is the LSP languageId a server for this format understands. */
export const LANGUAGES = {
  vega: { highlight: json, lsp: 'json' },
  vegalite: { highlight: json, lsp: 'json' },
  wireviz: { highlight: yaml, lsp: 'yaml' },
  d2: { highlight: null, lsp: 'd2' }, // no CodeMirror grammar; the server (via lsp-bridge /d2) supplies diagnostics, format, hover
  wavedrom: { highlight: json, lsp: null }, // highlight only: valid WaveDrom is not strict JSON
}

export const lspCapable = (format) => Boolean(LANGUAGES[format]?.lsp)
export const lspFormats = () => Object.keys(LANGUAGES).filter(lspCapable)

/** CodeMirror language extension for a format, or null for plain text. */
export function highlightFor(format) {
  const l = LANGUAGES[format]
  return l?.highlight ? l.highlight() : null
}

const STORAGE_KEY = 'kr0ki:lspUrls'

/** {format: wsUrl}. Only well-formed ws:// or wss:// URLs for LSP-capable formats are ever returned. */
export function loadLspUrls(storage = globalThis.localStorage) {
  try {
    const raw = JSON.parse(storage?.getItem(STORAGE_KEY) || '{}')
    return Object.fromEntries(Object.entries(raw).filter(([f, u]) => lspCapable(f) && isWsUrl(u)))
  } catch {
    return {}
  }
}

export function saveLspUrls(urls, storage = globalThis.localStorage) {
  const clean = Object.fromEntries(Object.entries(urls).filter(([f, u]) => lspCapable(f) && isWsUrl(u)))
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(clean))
  } catch { /* storage blocked: the setting just won't persist */ }
  return clean
}

export function isWsUrl(u) {
  try {
    const p = new URL(u)
    return p.protocol === 'ws:' || p.protocol === 'wss:'
  } catch {
    return false
  }
}

/**
 * @codemirror/lsp-client 6.3.0 declares `textDocument.diagnostic` (pull diagnostics) in its client capabilities but never
 * sends a `textDocument/diagnostic` request. Servers that honour the capability (vscode-json-language-server) then wait to be
 * asked and never push, so no diagnostics ever show. Strip the capability from `initialize` so servers fall back to
 * `publishDiagnostics`, which the client does handle. Applies to every outgoing message; only `initialize` is touched.
 */
export function patchOutgoing(message) {
  try {
    const m = JSON.parse(message)
    if (m?.method === 'initialize' && m.params?.capabilities?.textDocument?.diagnostic !== undefined) {
      delete m.params.capabilities.textDocument.diagnostic
      return JSON.stringify(m)
    }
  } catch { /* not JSON: pass through unchanged */ }
  return message
}

/** Open a WebSocket and wrap it as the LSP client's Transport (bare JSON strings; the bridge does the framing). */
export function wsTransport(url, { WebSocketImpl = globalThis.WebSocket, timeoutMs = 4000 } = {}) {
  return new Promise((resolve, reject) => {
    let socket
    try {
      socket = new WebSocketImpl(url)
    } catch (err) {
      return reject(err)
    }
    let handlers = []
    const timer = setTimeout(() => {
      try { socket.close() } catch { /* already closed */ }
      reject(new Error('timed out connecting'))
    }, timeoutMs)
    socket.onmessage = (e) => handlers.forEach((h) => h(String(e.data)))
    socket.onerror = () => { clearTimeout(timer); reject(new Error('connection failed')) }
    socket.onopen = () => {
      clearTimeout(timer)
      resolve({
        send: (message) => socket.send(patchOutgoing(message)),
        subscribe: (h) => handlers.push(h),
        unsubscribe: (h) => { handlers = handlers.filter((x) => x !== h) },
        close: () => { try { socket.close() } catch { /* ignore */ } },
        onClose: (fn) => { socket.onclose = fn },
      })
    }
  })
}

/**
 * Try to attach a language server. Resolves `{status: 'connected', extension, dispose}` or
 * `{status: 'unavailable', message}`; never throws. `extension` goes into the editor's extensions.
 */
export async function connectLsp(format, url, { documentUri, transportFactory = wsTransport, initTimeoutMs = 6000 } = {}) {
  const languageId = LANGUAGES[format]?.lsp
  if (!languageId) return { status: 'unavailable', message: `no language server is known for ${format}` }
  if (!isWsUrl(url)) return { status: 'unavailable', message: 'not a ws:// or wss:// URL' }
  let transport
  try {
    transport = await transportFactory(url)
    const client = new LSPClient({ extensions: languageServerExtensions() }).connect(transport)
    await Promise.race([
      client.initializing,
      new Promise((_, rej) => setTimeout(() => rej(new Error('server did not initialize')), initTimeoutMs)),
    ])
    const dispose = () => {
      try { client.disconnect() } catch { /* ignore */ }
      transport.close?.()
    }
    return { status: 'connected', languageId, extension: client.plugin(documentUri || `file:///kr0ki/source.${languageId}`, languageId), dispose }
  } catch (err) {
    transport?.close?.()
    return { status: 'unavailable', message: err?.message || 'connection failed' }
  }
}
