import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { Quasar } from 'quasar'
import App from './App.vue'

const catalog = {
  useCases: ['process flow'],
  types: [{ id: 'flowchart', name: 'Flowchart', syntax: 'd2', exampleId: 'e1', useCases: ['process flow'], blurb: 'Steps.', samplePrompt: 'p' }],
}
const examples = [{ id: 'e1', format: 'd2', title: 'Flow', description: 'd', input_kind: 'code', source: 'a -> b', outputs: ['svg'] }]
const StoryStub = { name: 'StoryB00k', props: { planner: Boolean, threadId: String }, template: '<div class="story-stub" :data-planner="planner" />' }

let renders
beforeEach(() => {
  renders = []
  Element.prototype.scrollIntoView = vi.fn()
  vi.stubGlobal('CSS', { escape: (s) => s })
  vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: () => 'blob:x', revokeObjectURL() {} }))
  vi.stubGlobal('fetch', vi.fn(async (url, init) => {
    const u = String(url)
    if (u.includes('/api/catalog')) return { ok: true, json: async () => catalog }
    if (u.includes('examples')) return { ok: true, json: async () => examples }
    if (u.includes('/render/')) { renders.push(u); return { ok: true, blob: async () => new Blob(['<svg/>']), text: async () => '<svg/>' } }
    return { ok: false, status: 404, json: async () => ({}), text: async () => '' }
  }))
})
afterEach(() => vi.unstubAllGlobals())

async function mountApp() {
  const w = mount(App, { global: { plugins: [[Quasar, {}]], stubs: { StoryB00k: StoryStub } }, attachTo: document.body })
  await flushPromises()
  return w
}

describe('App navigation', () => {
  it('has Planner and Projects as top-level menu items after Gallery; the gallery is home', async () => {
    const w = await mountApp()
    const labels = w.findAll('.view-tab').map((b) => b.text())
    expect(labels).toEqual(['Gallery', 'Planner', 'Projects', 'Assurance', 'Code Editor', 'Agent', 'Setup'])
    expect(w.find('[data-testid="tab-gallery"]').classes()).toContain('active')
    expect(w.find('.gallery').exists()).toBe(true)
    w.unmount()
  })

  it('shows the diagram-type tree in the sidebar, and selecting a type opens the editor and renders it', async () => {
    const w = await mountApp()
    expect(w.find('[data-testid="catalog-tree"]').exists()).toBe(true)
    await w.find('.tree-leaf[data-type-id="flowchart"]').trigger('click')
    await flushPromises()
    expect(w.find('[data-testid="tab-editor"]').classes()).toContain('active')
    expect(renders.some((u) => u.includes('/render/d2'))).toBe(true) // auto-render, no extra click
    w.unmount()
  })

  it('opens the planner page, keeps its chat mounted when you leave and come back, and moves nothing else', async () => {
    const w = await mountApp()
    expect(w.find('[data-testid="planner-view"]').exists()).toBe(false) // not mounted until first visited
    await w.find('[data-testid="tab-planner"]').trigger('click')
    expect(w.find('[data-testid="planner-view"]').isVisible()).toBe(true)
    expect(w.find('.story-stub').attributes('data-planner')).toBeDefined()
    await w.find('[data-testid="tab-setup"]').trigger('click')
    expect(w.find('[data-testid="planner-view"]').exists()).toBe(true) // still mounted (v-show), conversation survives
    expect(w.find('[data-testid="planner-view"]').isVisible()).toBe(false)
    await w.find('[data-testid="tab-planner"]').trigger('click')
    expect(w.find('[data-testid="planner-view"]').isVisible()).toBe(true)
    w.unmount()
  })
})

describe('App planner picks', () => {
  it('a tree click is not shown as the planner\'s best fit', async () => {
    const w = await mountApp()
    await w.find('[data-testid="tab-planner"]').trigger('click')
    await w.find('.tree-leaf[data-type-id="flowchart"]').trigger('click') // user navigates to the editor...
    await flushPromises()
    await w.find('[data-testid="tab-planner"]').trigger('click')
    expect(w.find('[data-testid="planner-view"]').findAll('.pick-card')).toHaveLength(0) // ...which is not a planner pick
    w.unmount()
  })
})
