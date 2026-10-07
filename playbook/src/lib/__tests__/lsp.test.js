import { describe, expect, it, vi } from 'vitest'
import { LANGUAGES, connectLsp, highlightFor, isWsUrl, loadLspUrls, lspCapable, lspFormats, patchOutgoing, saveLspUrls, wsTransport } from '../lsp.js'

const mem = (init = {}) => { const m = { ...init }; return { getItem: (k) => m[k] ?? null, setItem: (k, v) => { m[k] = String(v) }, dump: () => m } }

class FakeSocket {
  static last
  constructor(url) { this.url = url; this.sent = []; FakeSocket.last = this }
  send(m) { this.sent.push(m) }
  close() { this.closed = true }
}

describe('which languages get what', () => {
  it('offers a language server only for strict JSON and YAML formats, and highlighting where a grammar exists', () => {
    expect(lspFormats().sort()).toEqual(['d2', 'vega', 'vegalite', 'wireviz'])
    expect(lspCapable('wavedrom')).toBe(false) // JSON5-ish: a strict JSON server would flag valid sources
    expect(LANGUAGES.wavedrom.highlight).toBeTypeOf('function')
    expect(lspCapable('d2')).toBe(true)
    expect(highlightFor('d2')).toBeNull() // no CodeMirror grammar; plain text plus server marks
    expect(highlightFor('d2')).toBeNull()
    expect(highlightFor('vegalite')).toBeTruthy()
  })
})

describe('language-server settings', () => {
  it('keeps only well-formed ws:// URLs for capable formats and survives corrupt or blocked storage', () => {
    expect(isWsUrl('ws://127.0.0.1:8791/json') && isWsUrl('wss://h/x')).toBe(true)
    for (const bad of ['http://x', 'javascript:alert(1)', 'not a url', '']) expect(isWsUrl(bad)).toBe(false)
    const s = mem()
    expect(saveLspUrls({ vegalite: 'ws://h:1/json', graphviz: 'ws://h:1/g', vega: 'http://h' }, s)).toEqual({ vegalite: 'ws://h:1/json' })
    expect(loadLspUrls(s)).toEqual({ vegalite: 'ws://h:1/json' })
    expect(loadLspUrls(mem({ 'kr0ki:lspUrls': '{not json' }))).toEqual({})
    expect(loadLspUrls({ getItem() { throw new Error('blocked') } })).toEqual({})
  })
})

describe('transport', () => {
  it('strips the unused pull-diagnostics capability from initialize only', () => {
    const init = JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: { capabilities: { textDocument: { hover: {}, diagnostic: {}, publishDiagnostics: {} } } } })
    const out = JSON.parse(patchOutgoing(init))
    expect(out.params.capabilities.textDocument).toEqual({ hover: {}, publishDiagnostics: {} })
    const other = JSON.stringify({ method: 'textDocument/didOpen', params: { textDocument: { diagnostic: 1 } } })
    expect(patchOutgoing(other)).toBe(other)
    expect(patchOutgoing('not json')).toBe('not json')
    const plain = JSON.stringify({ method: 'initialize', params: { capabilities: {} } })
    expect(patchOutgoing(plain)).toBe(plain)
  })

  it('wraps a WebSocket as a Transport that patches what it sends and delivers what it receives', async () => {
    const p = wsTransport('ws://h:1/json', { WebSocketImpl: FakeSocket })
    FakeSocket.last.onopen()
    const t = await p
    const got = []; const h = (m) => got.push(m)
    t.subscribe(h); FakeSocket.last.onmessage({ data: 'one' }); t.unsubscribe(h); FakeSocket.last.onmessage({ data: 'two' })
    expect(got).toEqual(['one'])
    t.send(JSON.stringify({ method: 'initialize', params: { capabilities: { textDocument: { diagnostic: {} } } } }))
    expect(JSON.parse(FakeSocket.last.sent[0]).params.capabilities.textDocument.diagnostic).toBeUndefined()
  })

  it('rejects when the socket errors or never opens', async () => {
    const p = wsTransport('ws://h:1/json', { WebSocketImpl: FakeSocket })
    FakeSocket.last.onerror()
    await expect(p).rejects.toThrow('connection failed')
    vi.useFakeTimers()
    const slow = wsTransport('ws://h:1/json', { WebSocketImpl: FakeSocket, timeoutMs: 50 })
    const assertion = expect(slow).rejects.toThrow('timed out')
    await vi.advanceTimersByTimeAsync(60)
    await assertion
    vi.useRealTimers()
  })
})

describe('connectLsp', () => {
  it('never throws: unknown formats, bad URLs and unreachable servers all come back as unavailable', async () => {
    expect((await connectLsp('d2', 'ws://h:1/json')).status).toBe('unavailable')
    expect((await connectLsp('vegalite', 'http://h')).message).toContain('ws://')
    const refused = await connectLsp('vegalite', 'ws://h:1/json', { transportFactory: async () => { throw new Error('connection failed') } })
    expect(refused).toEqual({ status: 'unavailable', message: 'connection failed' })
  })

  it('reports unavailable (and closes the socket) when the server never answers initialize', async () => {
    const closed = vi.fn()
    const transport = { send() {}, subscribe() {}, unsubscribe() {}, close: closed }
    const r = await connectLsp('vegalite', 'ws://h:1/json', { transportFactory: async () => transport, initTimeoutMs: 40 })
    expect(r.status).toBe('unavailable')
    expect(r.message).toContain('did not initialize')
    expect(closed).toHaveBeenCalled()
  })
})
