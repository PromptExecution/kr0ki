// A tiny stdio "language server" for the bridge tests: replies to initialize (reporting its pid and cwd) and publishes a
// diagnostic on didOpen. Speaks Content-Length framing like a real one, and can emit a hostile frame on request.
let buf = Buffer.alloc(0)
const send = (obj) => { const s = typeof obj === 'string' ? obj : JSON.stringify(obj); process.stdout.write(`Content-Length: ${Buffer.byteLength(s)}\r\n\r\n${s}`) }
process.stdin.on('data', (chunk) => {
  buf = Buffer.concat([buf, chunk])
  for (;;) {
    const he = buf.indexOf('\r\n\r\n'); if (he < 0) break
    const len = Number(/content-length:\s*(\d+)/i.exec(buf.subarray(0, he).toString())[1])
    if (buf.length < he + 4 + len) break
    const msg = JSON.parse(buf.subarray(he + 4, he + 4 + len).toString('utf8')); buf = buf.subarray(he + 4 + len)
    if (msg.method === 'initialize') send({ jsonrpc: '2.0', id: msg.id, result: { capabilities: {}, serverInfo: { name: 'fake', pid: process.pid, cwd: process.cwd(), envKeys: Object.keys(process.env).sort() } } })
    else if (msg.method === 'textDocument/didOpen') send({ jsonrpc: '2.0', method: 'textDocument/publishDiagnostics', params: { uri: msg.params.textDocument.uri, diagnostics: [{ message: 'héllo ✓', range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } } }] } })
    else if (msg.method === 'x/bomb') process.stdout.write('Content-Length: 99999999\r\n\r\n')
  }
})
