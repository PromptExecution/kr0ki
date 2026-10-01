// jsdom has no screen.orientation, which Quasar reads when it installs. Provide the minimum.
if (typeof window !== 'undefined' && window.screen && !window.screen.orientation) {
  Object.defineProperty(window.screen, 'orientation', { value: { type: 'landscape-primary', angle: 0, addEventListener() {}, removeEventListener() {} }, configurable: true })
}

// CodeMirror measures text with Range#getClientRects / getBoundingClientRect, which jsdom lacks.
if (typeof document !== 'undefined') {
  const empty = () => ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} })
  const rect = () => ({ x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, toJSON() { return this } })
  const proto = Range.prototype
  if (!proto.getClientRects) proto.getClientRects = empty
  if (!proto.getBoundingClientRect) proto.getBoundingClientRect = rect
}
