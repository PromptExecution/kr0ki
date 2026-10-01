// jsdom has no screen.orientation, which Quasar reads when it installs. Provide the minimum.
if (typeof window !== 'undefined' && window.screen && !window.screen.orientation) {
  Object.defineProperty(window.screen, 'orientation', { value: { type: 'landscape-primary', angle: 0, addEventListener() {}, removeEventListener() {} }, configurable: true })
}
