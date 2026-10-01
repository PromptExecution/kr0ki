import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import RendererPanel from './RendererPanel.vue'
import { clearSkillCache } from '../lib/skills.js'

const d2 = { id: 'e1', format: 'd2', title: 'Flow', description: 'd', input_kind: 'code', source: 'a -> b', outputs: ['svg'] }
const gv = { id: 'e2', format: 'graphviz', title: 'Graph', description: 'g', input_kind: 'code', source: 'digraph{a->b}', outputs: ['svg'] }
const k8s = { id: 'e3', format: 'k8s-topology', title: 'K8s', description: 'k', input_kind: 'yaml', source: 'kind: Pod', outputs: ['svg'], route: '/render/k8s-topology' }
const skills = { d2: '---\nname: kr0ki-d2\n---\n# d2\nQuote labels that contain $.', graphviz: '# graphviz\nRecord shapes.' }

let skillCalls
beforeEach(() => {
  clearSkillCache()
  skillCalls = []
  vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: () => 'blob:x', revokeObjectURL() {} }))
  vi.stubGlobal('fetch', vi.fn(async (url) => {
    const u = String(url)
    const m = u.match(/\/skills\/diagrams\/([^/?]+)$/)
    if (m) {
      skillCalls.push(m[1])
      return skills[m[1]] ? { ok: true, status: 200, text: async () => skills[m[1]] } : { ok: false, status: 404, text: async () => '' }
    }
    return { ok: true, status: 200, blob: async () => new Blob(['<svg/>']), text: async () => '<svg/>' }
  }))
})
afterEach(() => vi.unstubAllGlobals())

const mountPanel = (example = d2, extra = {}) =>
  mount(RendererPanel, { props: { example, examples: [d2, gv, k8s], rendererUrl: 'http://k.test:8787', agentUrl: 'http://a.test:8789', ...extra } })

describe('RendererPanel syntax skill', () => {
  it('shows the language skill in a monospace block directly below the source editor', async () => {
    const w = mountPanel()
    await flushPromises()
    const col = w.find('.editor-col')
    const kids = [...col.element.children].map((c) => c.className)
    expect(kids).toEqual(['source-label', 'skill-panel']) // editor first (top), skill below it
    expect(w.find('[data-testid="skill-text"]').text()).toContain('Quote labels that contain $.')
    expect(w.find('[data-testid="skill-panel"]').text()).toContain('Syntax skill · d2')
    expect(w.find('[data-testid="skill-text"]').element.tagName).toBe('PRE')
    expect(skillCalls).toEqual(['d2'])
  })

  it('swaps the skill when the example changes to another format, and ignores a slow answer for the old one', async () => {
    const w = mountPanel()
    await flushPromises()
    await w.setProps({ example: gv })
    await flushPromises()
    expect(w.find('[data-testid="skill-text"]').text()).toContain('Record shapes.')
    expect(w.find('[data-testid="skill-panel"]').text()).toContain('graphviz')
    expect(skillCalls).toEqual(['d2', 'graphviz'])
  })

  it('says so when a format has no skill, and when the agent cannot be reached', async () => {
    const w = mountPanel(k8s)
    await flushPromises()
    expect(w.find('[data-testid="skill-missing"]').text()).toContain('No syntax skill for k8s-topology')
    vi.stubGlobal('fetch', vi.fn(async () => { throw new Error('connection refused') }))
    clearSkillCache()
    const down = mountPanel(d2)
    await flushPromises()
    expect(down.find('[data-testid="skill-error"]').text()).toContain('connection refused')
  })

  it('does not break the editor: the textarea is still the source and stays editable', async () => {
    const w = mountPanel()
    await flushPromises()
    const ta = w.find('[data-testid="source-editor"]')
    expect(ta.element.value).toContain('a -> b')
    await ta.setValue('x -> y')
    expect(ta.element.value).toBe('x -> y')
  })
})
