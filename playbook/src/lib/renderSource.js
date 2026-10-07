// Render diagram source through the kr0ki server and return the SVG text. Mirrors the editor's request (POST text/plain to
// `/render/{format}` or the example's own route), so what the agent panel shows is what the editor would render.
export async function renderSvg({ rendererUrl, format, route = null, source, fetchImpl = globalThis.fetch }) {
  const base = String(rendererUrl || '').trim().replace(/\/$/, '')
  if (!base) throw new Error('Set the kr0ki URL in Setup first')
  if (!format) throw new Error('No diagram format')
  const res = await fetchImpl(`${base}${route || `/render/${format}`}?output=svg`, {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body: source,
  })
  const text = await res.text()
  if (!res.ok) throw new Error(text.slice(0, 400) || `HTTP ${res.status}`)
  return text
}

/** Trigger a browser download of text (diagram code or SVG). Returns the filename used. */
export function downloadText(filename, text, type = 'text/plain') {
  const url = URL.createObjectURL(new Blob([text], { type }))
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
  return filename
}

const EXT = { d2: 'd2', graphviz: 'dot', plantuml: 'puml', c4plantuml: 'puml', mermaid: 'mmd', 'k8s-topology': 'yaml', vega: 'json', vegalite: 'json', wireviz: 'yaml' }
export const sourceFilename = (format, stem = 'diagram') => `${stem}.${EXT[format] || format || 'txt'}`
