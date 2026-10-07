import { describe, expect, it, vi } from 'vitest'
import { bindViewHistory, hashToView, viewToHash } from '../viewHistory.js'

function fakeWin(hash = '') {
  const listeners = {}
  const w = {
    location: { hash },
    history: {
      stack: [],
      replaceState: vi.fn((s, _t, h) => { w.location.hash = h }),
      pushState: vi.fn((s, _t, h) => { w.history.stack.push(h); w.location.hash = h }),
    },
    addEventListener: (e, f) => { listeners[e] = f },
    removeEventListener: (e) => { delete listeners[e] },
    fire: (e) => listeners[e]?.(),
  }
  return w
}

describe('view history', () => {
  it('maps views to hashes and back (agent is the slug for storyb00k)', () => {
    expect(viewToHash('storyb00k')).toBe('#/agent')
    expect(viewToHash('editor')).toBe('#/editor')
    expect(hashToView('#/agent')).toBe('storyb00k')
    expect(hashToView('#/editor?x=1')).toBe('editor')
    expect(hashToView('#/nonsense')).toBeNull()
    expect(hashToView('')).toBeNull()
  })

  it('pushes one entry per view change and follows Back', () => {
    const win = fakeWin()
    let view = 'gallery'
    const h = bindViewHistory({ getView: () => view, setView: (v) => { view = v }, win })
    expect(win.location.hash).toBe('#/gallery')
    view = 'editor'; h.push('editor')
    view = 'storyb00k'; h.push('storyb00k')
    expect(win.history.stack).toEqual(['#/editor', '#/agent'])
    win.location.hash = '#/editor'; win.fire('popstate') // the browser went Back
    expect(view).toBe('editor')
    h.push('editor') // already the current entry: no duplicate
    expect(win.history.pushState).toHaveBeenCalledTimes(2)
    h.stop()
  })

  it('opens a deep link', () => {
    const win = fakeWin('#/assurance')
    let view = 'gallery'
    bindViewHistory({ getView: () => view, setView: (v) => { view = v }, win })
    expect(view).toBe('assurance')
  })
})
