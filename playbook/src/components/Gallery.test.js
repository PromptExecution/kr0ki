import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import Gallery from './Gallery.vue'

const catalog = {
  useCases: ['process flow', 'data model'],
  types: [
    { id: 'flowchart', name: 'Flowchart', syntax: 'd2', exampleId: 'e1', useCases: ['process flow'], blurb: 'Steps.', samplePrompt: 'p' },
    { id: 'er', name: 'ER diagram', syntax: 'plantuml', exampleId: 'e2', useCases: ['data model'], blurb: 'Tables.', samplePrompt: 'p' },
  ],
}

let fetched
beforeEach(() => {
  fetched = []
  vi.stubGlobal('fetch', vi.fn(async (url) => { fetched.push(String(url)); return { ok: true, json: async () => catalog } }))
  Element.prototype.scrollIntoView = vi.fn()
  vi.stubGlobal('CSS', { escape: (s) => s }) // jsdom has no CSS.escape
})
afterEach(() => vi.unstubAllGlobals())

const StoryStub = { name: 'StoryB00k', props: { planner: Boolean, threadId: String, agentUrl: String }, template: '<div data-testid="story-stub" />' }

async function mountGallery(props = {}) {
  const w = mount(Gallery, {
    props: { examples: [], rendererUrl: 'http://kr0ki.test:8787', plannerSession: 'planner-abc', agentUrl: 'http://agent.test:8789', ...props },
    global: { stubs: { StoryB00k: StoryStub } },
  })
  await flushPromises()
  return w
}

describe('Gallery planner', () => {
  it('has no Renderer URL field and reads the catalog from the configured service', async () => {
    const w = await mountGallery()
    expect(w.find('input[aria-label="Renderer URL"]').exists()).toBe(false)
    expect(w.text()).not.toContain('Renderer URL')
    expect(fetched[0]).toBe('http://kr0ki.test:8787/api/catalog')
  })

  it('hosts the planner chat on the session thread, and says how an MCP client can steer the page', async () => {
    const w = await mountGallery()
    const chat = w.findComponent(StoryStub)
    expect(chat.props()).toMatchObject({ planner: true, threadId: 'planner-abc', agentUrl: 'http://agent.test:8789' })
    expect(w.find('[data-testid="planner-session"]').text()).toContain('planner-abc')
    await w.find('[data-testid="planner-toggle"]').trigger('click')
    expect(w.findComponent(StoryStub).exists()).toBe(false)
  })

  it('shows the planner status and shortlist, marking only suggested cards', async () => {
    const w = await mountGallery({ uiStatus: 'open', suggested: ['er'], suggestNote: 'you described tables' })
    expect(w.find('.planner-link').text()).toContain('page linked')
    expect(w.find('[data-testid="planner-suggestion"]').text()).toContain('ER diagram')
    expect(w.find('[data-testid="planner-suggestion"]').text()).toContain('you described tables')
    const badges = w.findAll('[data-testid="suggested-badge"]')
    expect(badges).toHaveLength(1)
    expect(w.find('[data-type-id="er"]').classes()).toContain('suggested')
    expect(w.find('[data-type-id="flowchart"]').classes()).not.toContain('suggested')
  })

  it('shows no shortlist banner when nothing is suggested', async () => {
    const w = await mountGallery()
    expect(w.find('[data-testid="planner-suggestion"]').exists()).toBe(false)
  })

  it('never lets a planner selection hide behind the current filter', async () => {
    const w = await mountGallery({ useCase: 'process flow', selectedTypeId: '' })
    expect(w.find('[data-type-id="er"]').exists()).toBe(false)
    await w.setProps({ selectedTypeId: 'er' })
    expect(w.emitted('update:useCase')?.at(-1)).toEqual(['All'])
  })

  it('lets the user filter and select, reporting both upward', async () => {
    const w = await mountGallery()
    await w.findAll('.gallery-filter').find((b) => b.text().startsWith('data model')).trigger('click')
    expect(w.emitted('update:useCase').at(-1)).toEqual(['data model'])
    await w.find('[data-type-id="er"]').trigger('click')
    expect(w.emitted('update:selectedTypeId').at(-1)).toEqual(['er'])
  })
})
