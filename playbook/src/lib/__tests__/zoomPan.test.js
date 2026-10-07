import { describe, expect, it } from 'vitest'
import { MAX_SCALE, MIN_SCALE, clampScale, fit, panBy, svgSize, zoomAt } from '../zoomPan.js'

describe('zoomPan', () => {
  it('zooms about the cursor: the point under it does not move', () => {
    const v = { scale: 1, x: 10, y: 20 }
    const z = zoomAt(v, 2, 110, 120) // content point under cursor: (100, 100)
    expect(z.scale).toBe(2)
    expect(100 * z.scale + z.x).toBeCloseTo(110)
    expect(100 * z.scale + z.y).toBeCloseTo(120)
  })
  it('clamps the scale', () => {
    expect(clampScale(1e9)).toBe(MAX_SCALE)
    expect(clampScale(0)).toBe(MIN_SCALE)
    expect(zoomAt({ scale: MAX_SCALE, x: 0, y: 0 }, 2, 5, 5).scale).toBe(MAX_SCALE)
  })
  it('fits a large diagram and centres a small one without enlarging it', () => {
    const big = fit(4000, 2000, 800, 600)
    expect(big.scale).toBeCloseTo((800 - 32) / 4000)
    expect(big.x).toBeCloseTo(16)
    const small = fit(100, 50, 800, 600)
    expect(small.scale).toBe(1)
    expect(small.x).toBe(350)
    expect(fit(100, 50, 800, 600, { allowEnlarge: true }).scale).toBeGreaterThan(1)
    expect(fit(0, 0, 800, 600)).toEqual({ scale: 1, x: 0, y: 0 })
  })
  it('pans', () => expect(panBy({ scale: 2, x: 1, y: 2 }, 5, -5)).toEqual({ scale: 2, x: 6, y: -3 }))
  it('reads the natural size of an svg', () => {
    expect(svgSize('<svg xmlns="x" viewBox="0 0 1200.5 800">')).toEqual({ w: 1200.5, h: 800 })
    expect(svgSize('<svg width="300px" height="200">')).toEqual({ w: 300, h: 200 })
    expect(svgSize('<svg>')).toBeNull()
    expect(svgSize('nothing')).toBeNull()
  })
})
