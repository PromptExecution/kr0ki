import { describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ZoomPan from './ZoomPan.vue'
import CheckpointDialog from './CheckpointDialog.vue'
import { createProjectStore } from '../lib/projects.js'

function mem() {
  const m = new Map()
  return { getItem: (k) => (m.has(k) ? JSON.parse(m.get(k)) : null), set: (k, v) => m.set(k, JSON.stringify(v)), remove: (k) => m.delete(k), getAllKeys: () => [...m.keys()] }
}

// jsdom has no PointerEvent/WheelEvent that accepts these fields through trigger(); build the events by hand.
function fire(el, type, props) {
  const e = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: props.clientX ?? 0, clientY: props.clientY ?? 0, ctrlKey: !!props.ctrlKey })
  for (const k of ['pointerId', 'deltaY']) if (k in props) Object.defineProperty(e, k, { value: props[k] })
  el.dispatchEvent(e)
}

describe('ZoomPan', () => {
  const mountZp = () => mount(ZoomPan, { slots: { default: '<svg viewBox="0 0 4000 2000"></svg>' }, attachTo: document.body })

  it('zooms with the buttons and the keyboard, and resets to 100%', async () => {
    const w = mountZp()
    const pct = () => w.find('[data-testid="zp-pct"]').text()
    await w.find('[data-testid="zp-100"]').trigger('click')
    expect(pct()).toBe('100%')
    await w.find('[data-testid="zp-in"]').trigger('click')
    expect(pct()).toBe('125%')
    await w.find('[data-testid="zp-out"]').trigger('click')
    await w.find('[data-testid="zp-out"]').trigger('click')
    expect(pct()).toBe('80%')
    await w.find('.zp__viewport').trigger('keydown', { key: '+' })
    expect(pct()).toBe('100%')
    await w.find('.zp__viewport').trigger('keydown', { key: '0' })
    expect(pct()).toBe('100%')
    w.unmount()
  })

  it('pans by dragging', async () => {
    const w = mountZp()
    const vp = w.find('.zp__viewport')
    const before = w.vm.view.x
    fire(vp.element, 'pointerdown', { pointerId: 1, clientX: 100, clientY: 100 })
    fire(vp.element, 'pointermove', { pointerId: 1, clientX: 160, clientY: 130 })
    fire(vp.element, 'pointerup', { pointerId: 1 })
    expect(w.vm.view.x).toBe(before + 60)
    w.unmount()
  })

  it('leaves the plain wheel to the page and zooms on Ctrl+wheel', async () => {
    const w = mountZp()
    const before = w.vm.view.scale
    fire(w.find('.zp__viewport').element, 'wheel', { deltaY: -100 })
    expect(w.vm.view.scale).toBe(before)
    fire(w.find('.zp__viewport').element, 'wheel', { deltaY: -100, ctrlKey: true })
    expect(w.vm.view.scale).toBeGreaterThan(before)
    w.unmount()
  })

  it('offers a larger viewing area', async () => {
    const w = mountZp()
    await w.find('[data-testid="zp-tall"]').trigger('click')
    expect(w.find('.zp__viewport').attributes('style')).toContain('80vh')
    w.unmount()
  })
})

describe('CheckpointDialog', () => {
  it('saves a checkpoint into a new project and says how to recover it', async () => {
    const store = createProjectStore({ storage: mem() })
    const w = mount(CheckpointDialog, { props: { store, source: 'a -> b', format: 'd2', title: 'My Flow' } })
    await w.find('[data-testid="cp-toggle"]').trigger('click')
    expect(w.find('[data-testid="cp-path"]').element.value).toBe('diagrams/my-flow.d2')
    await w.find('[data-testid="cp-name"]').setValue('Agents')
    await w.find('form').trigger('submit')
    expect(w.find('[data-testid="cp-notice"]').text()).toContain('Recover it in Projects')
    const [p] = store.listProjects()
    expect(p.name).toBe('Agents')
    expect(store.checkout(p.id, p.head)).toEqual({ 'diagrams/my-flow.d2': 'a -> b' })
    expect(w.emitted('saved')).toHaveLength(1)
  })

  it('adds to an existing project and reports an identical save plainly', async () => {
    const store = createProjectStore({ storage: mem() })
    const p = store.createProject({ name: 'Existing' })
    const w = mount(CheckpointDialog, { props: { store, source: 'x -> y', format: 'd2', title: 't' } })
    await w.find('[data-testid="cp-toggle"]').trigger('click')
    await w.find('form').trigger('submit')
    await w.find('form').trigger('submit')
    expect(w.find('[data-testid="cp-notice"]').text()).toContain('Nothing to save')
    expect(store.log(p.id)).toHaveLength(1)
  })

  it('is disabled with no code and shows storage errors instead of failing silently', async () => {
    const empty = mount(CheckpointDialog, { props: { store: createProjectStore({ storage: mem() }), source: '  ', format: 'd2' } })
    expect(empty.find('[data-testid="cp-toggle"]').attributes('disabled')).toBeDefined()
    const bad = { listProjects: () => [], createProject: () => { throw new Error('The browser storage is full.') } }
    const w = mount(CheckpointDialog, { props: { store: bad, source: 'a', format: 'd2' } })
    await w.find('[data-testid="cp-toggle"]').trigger('click')
    await w.find('form').trigger('submit')
    expect(w.find('[data-testid="cp-notice"]').text()).toContain('storage is full')
  })
})
