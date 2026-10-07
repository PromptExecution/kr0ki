import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import StoryB00k from './StoryB00k.vue'
import RevisionTimeline from './RevisionTimeline.vue'
import { createRevisionGraph, addPromptNode } from '../lib/revisionGraph.js'

const handoff = { source: 'a -> b', format: 'd2', detectedType: 'd2', title: 'Demo', imageData: null }
let calls
beforeEach(() => {
  calls = []
  vi.stubGlobal('fetch', vi.fn(async (url, init) => {
    calls.push({ url: String(url), init })
    if (String(url).endsWith('/health')) return { ok: true, status: 200, json: async () => ({}), text: async () => '' }
    if (String(url).includes('/render/')) return { ok: true, status: 200, text: async () => '<svg id="rendered"/>' }
    return { ok: false, status: 404, json: async () => ({}), text: async () => '' }
  }))
})
afterEach(() => vi.unstubAllGlobals())

describe('the agent panel: starting point', () => {
  it('can be edited, rendered in place, and applied as a new revision', async () => {
    const w = mount(StoryB00k, { props: { editorHandoff: handoff, rendererUrl: 'http://k.test:8787', agentUrl: 'http://a.test' } })
    await flushPromises()
    await w.find('[data-testid="starting-source"]').setValue('a -> b\nb -> c')
    await w.find('[data-testid="start-render"]').trigger('click')
    await flushPromises()
    expect(calls.some((c) => c.url === 'http://k.test:8787/render/d2?output=svg' && c.init.body === 'a -> b\nb -> c')).toBe(true)
    expect(w.find('[data-testid="starting-render"]').html()).toContain('rendered')
    await w.find('[data-testid="start-apply"]').trigger('click')
    expect(w.find('.storyb00k__rev-active').text()).toContain('Edited starting point')
    expect(w.find('[data-testid="start-apply"]').attributes('disabled')).toBeDefined() // already current
  })

  it('opens in the editor with the edited code', async () => {
    const w = mount(StoryB00k, { props: { editorHandoff: handoff, rendererUrl: 'http://k', agentUrl: 'http://a.test' } })
    await flushPromises()
    await w.find('[data-testid="starting-source"]').setValue('x -> y')
    await w.find('[data-testid="start-editor"]').trigger('click')
    expect(w.emitted('edit-in-editor')[0][0]).toMatchObject({ source: 'x -> y', format: 'd2' })
  })

  it('says why a render failed instead of staying silent', async () => {
    vi.stubGlobal('fetch', vi.fn(async (url) => (String(url).includes('/render/') ? { ok: false, status: 400, text: async () => 'syntax error line 1' } : { ok: true, text: async () => '' })))
    const w = mount(StoryB00k, { props: { editorHandoff: handoff, rendererUrl: 'http://k', agentUrl: 'http://a.test' } })
    await flushPromises()
    await w.find('[data-testid="start-render"]').trigger('click')
    await flushPromises()
    expect(w.find('[data-testid="start-render-state"]').text()).toContain('syntax error line 1')
  })
})

describe('the agent panel: what the buttons do', () => {
  it('labels the panel and chat buttons for what they actually do', async () => {
    const w = mount(StoryB00k, { props: { agentUrl: 'http://a.test' } })
    await flushPromises()
    const labels = w.findAll('button').map((b) => b.text())
    expect(labels).toContain('Clear chat')
    expect(labels.some((l) => l.includes('Context'))).toBe(true)
    expect(w.find('[title*="transcript"]').attributes('title')).toContain('revisions are kept')
  })

  it('shows what the agent receives, with nothing hidden', async () => {
    const w = mount(StoryB00k, { props: { editorHandoff: handoff, agentUrl: 'http://a.test' } })
    await flushPromises()
    await w.find('[data-testid="toggle-context"]').trigger('click')
    const text = w.find('[data-testid="context-info"]').text()
    expect(text).toContain('Sent with every message')
    expect(text).toContain('current diagram: d2, 1 lines')
    expect(text).toContain('Kept on the server')
  })
})

describe('RevisionTimeline', () => {
  function graph() {
    const g = createRevisionGraph({ source: 'a -> b', format: 'd2', label: 'Start' })
    addPromptNode(g, { prompt: 'add c', source: 'a -> b\nb -> c', format: 'd2', rendered: { svg: '<svg id="snap"/>' }, toolCallId: 't1', toolName: 'render_diagram' })
    addPromptNode(g, { prompt: 'add mcp', source: 'a -> b\nb -> c\nc -> mcp', format: 'd2', toolCallId: 't2', toolName: 'render_diagram' })
    return g
  }
  const mountTl = (g, extra = {}) => mount(RevisionTimeline, { props: { graph: g, toolCalls: { t1: { ok: true }, t2: { ok: false } }, ...extra } })

  it('lists every revision with what changed and whether its tool call succeeded', () => {
    const w = mountTl(graph())
    expect(w.findAll('li')).toHaveLength(3)
    expect(w.find('[data-testid="rev-1"]').text()).toContain('+1')
    expect(w.find('[data-testid="rev-1"]').text()).toContain('✓ succeeded')
    expect(w.find('[data-testid="rev-2"]').text()).toContain('✗ failed')
    expect(w.find('[data-testid="rev-2"]').text()).toContain('current')
  })

  it('shows the stored snapshot and restores an earlier revision without deleting later ones', async () => {
    const g = graph()
    const w = mountTl(g)
    await w.find('[data-testid="rev-1"] [data-testid="rev-view"]').trigger('click')
    expect(w.find('[data-testid="rev-snapshot"]').html()).toContain('snap')
    await w.find('[data-testid="rev-1"] [data-testid="rev-restore"]').trigger('click')
    expect(w.emitted('restore')).toEqual([[g.nodes[1].id]])
    expect(g.nodes).toHaveLength(3)
  })

  it('compares two revisions line by line', async () => {
    const w = mountTl(graph())
    await w.find('[data-testid="rev-0"] [data-testid="rev-compare"]').trigger('click')
    const text = w.find('[data-testid="rev-diff"]').text()
    expect(text).toContain('+ b -> c')
    expect(text).toContain('+ c -> mcp')
  })

  it('opens the revision a tool call produced', async () => {
    const g = graph()
    const w = mountTl(g, { focusId: g.nodes[1].id })
    expect(w.find('[data-testid="rev-snapshot"]').exists()).toBe(true)
  })

  it('draws a snapshot on demand for a revision that was never rendered', async () => {
    const g = graph()
    const f = vi.fn(async () => ({ ok: true, text: async () => '<svg id="late"/>' }))
    vi.stubGlobal('fetch', f)
    const w = mountTl(g, { focusId: g.nodes[2].id, rendererUrl: 'http://k.test' })
    await w.find('[data-testid="rev-render"]').trigger('click')
    await flushPromises()
    expect(f).toHaveBeenCalledWith('http://k.test/render/d2?output=svg', expect.anything())
    expect(w.find('[data-testid="rev-snapshot"]').html()).toContain('late')
  })
})
