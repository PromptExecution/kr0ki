// A fake stdio MCP server (newline-delimited JSON-RPC) for the bridge tests.
import readline from 'node:readline'
const out = (o) => process.stdout.write(JSON.stringify(o) + '\n')
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const m = JSON.parse(line)
  if (m.method === 'initialize') out({ jsonrpc: '2.0', id: m.id, result: { serverInfo: { name: 'fake', pid: process.pid, envKeys: Object.keys(process.env).sort() }, capabilities: {} } })
  else if (m.method === 'tools/list') { process.stdout.write('this is a stray log line, not JSON\n'); out({ jsonrpc: '2.0', id: m.id, result: { tools: [{ name: 'validate' }] } }) }
  else if (m.method === 'x/crash') process.exit(3)
  else if (m.method === 'x/silent') { /* never answers */ }
  else if (m.method === 'x/slow') setTimeout(() => out({ jsonrpc: '2.0', id: m.id, result: 'slow' }), 300)
  else if (m.id !== undefined) out({ jsonrpc: '2.0', id: m.id, result: { echo: m.method } })
})
