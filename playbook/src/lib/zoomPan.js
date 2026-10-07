// Pure view-transform maths for zooming and panning a large diagram: {scale, x, y} maps content to the viewport
// (screen = content * scale + translate). Kept apart from the component so it can be tested without a DOM.
export const MIN_SCALE = 0.01
export const MAX_SCALE = 16

export const clampScale = (s) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, s))

/** Zoom by `factor` keeping the content point under (px, py) -- viewport coordinates -- fixed. */
export function zoomAt(view, factor, px, py) {
  const scale = clampScale(view.scale * factor)
  const k = scale / view.scale
  return { scale, x: px - (px - view.x) * k, y: py - (py - view.y) * k }
}

/** Scale and centre content of (cw, ch) inside a viewport of (vw, vh); never enlarges past 100% unless `allowEnlarge`. */
export function fit(cw, ch, vw, vh, { padding = 16, allowEnlarge = false } = {}) {
  if (!(cw > 0) || !(ch > 0) || !(vw > 0) || !(vh > 0)) return { scale: 1, x: 0, y: 0 }
  let scale = Math.min((vw - 2 * padding) / cw, (vh - 2 * padding) / ch)
  if (!allowEnlarge) scale = Math.min(scale, 1)
  scale = clampScale(scale)
  return { scale, x: (vw - cw * scale) / 2, y: (vh - ch * scale) / 2 }
}

export const panBy = (view, dx, dy) => ({ ...view, x: view.x + dx, y: view.y + dy })

/** Natural size of an SVG string (viewBox, else width/height); null when it declares neither. */
export function svgSize(svgText) {
  const head = /<svg\b[^>]*>/i.exec(svgText || '')?.[0]
  if (!head) return null
  const vb = /viewBox\s*=\s*["']\s*([-\d.eE+]+)[\s,]+([-\d.eE+]+)[\s,]+([-\d.eE+]+)[\s,]+([-\d.eE+]+)/i.exec(head)
  if (vb && +vb[3] > 0 && +vb[4] > 0) return { w: +vb[3], h: +vb[4] }
  const w = /\swidth\s*=\s*["']([\d.]+)(?:px)?["']/i.exec(head)
  const h = /\sheight\s*=\s*["']([\d.]+)(?:px)?["']/i.exec(head)
  return w && h ? { w: +w[1], h: +h[1] } : null
}
