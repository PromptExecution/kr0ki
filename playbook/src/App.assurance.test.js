import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { Quasar } from 'quasar'
import App from './App.vue'

const catalog = { useCases: ['process flow'], types: [{ id: 'flowchart', name: 'Flowchart', syntax: 'd2', exampleId: 'e1', useCases: ['process flow'], blurb: 'Steps.', samplePrompt: 'p' }] }
const examples = [{ id: 'e1', format: 'd2', title: 'Flow', description: 'd', input_kind: 'code', source: 'a -> b', outputs: ['svg'] }]
const StoryStub = { name: 'StoryB00k', props: { planner: Boolean, threadId: String }, template: '<div />' }
// The real view is covered by AssuranceView.test.js; here we only check what App hands it.
const AssuranceStub = {
  name: 'AssuranceView',
  props: { rendererUrl: String, token: String },
  template: '<div class="assurance-stub" :data-url="rendererUrl" :data-token="token" />',
}

beforeEach(() => {
  sessionStorage.clear()
  localStorage.clear()
  Element.prototype.scrollIntoView = vi.fn()
  vi.stubGlobal('CSS', { escape: (s) => s })
  vi.stubGlobal('fetch', vi.fn(async (url) => {
    const u = String(url)
    if (u.includes('/api/catalog')) return { ok: true, json: async () => catalog }
    if (u.includes('examples')) return { ok: true, json: async () => examples }
    return { ok: false, status: 404, json: async () => ({}), text: async () => '' }
  }))
})
afterEach(() => { vi.unstubAllGlobals(); document.body.replaceChildren() })

async function mountApp() {
  const w = mount(App, { global: { plugins: [[Quasar, {}]], stubs: { StoryB00k: StoryStub, AssuranceView: AssuranceStub } }, attachTo: document.body })
  await flushPromises()
  return w
}
const t = (w, id) => w.find(`[data-testid="${id}"]`)

describe('the Assurance tab', () => {
  it('is in the menu and opens the assurance view with the renderer url', async () => {
    localStorage.setItem('kr0ki:rendererUrl', 'http://k.test:8787')
    const w = await mountApp()
    expect(t(w, 'tab-assurance').text()).toBe('Assurance')
    expect(w.find('.assurance-stub').exists()).toBe(false)
    await t(w, 'tab-assurance').trigger('click')
    const stub = w.find('.assurance-stub')
    expect(stub.exists()).toBe(true)
    expect(stub.attributes('data-url')).toBe('http://k.test:8787')
    expect(t(w, 'tab-assurance').classes()).toContain('active')
  })

  it('hides the gallery hero while the assurance view is open', async () => {
    const w = await mountApp()
    expect(w.find('.hero').exists()).toBe(true)
    await t(w, 'tab-assurance').trigger('click')
    expect(w.find('.hero').exists()).toBe(false)
  })

  it('leaves no assurance view behind when the person goes elsewhere', async () => {
    const w = await mountApp()
    await t(w, 'tab-assurance').trigger('click')
    await t(w, 'tab-gallery').trigger('click')
    expect(w.find('.assurance-stub').exists()).toBe(false)
  })
})

describe('the access token', () => {
  it('starts from the session, not from persistent storage', async () => {
    sessionStorage.setItem('kr0ki:token', 'tok-from-session')
    const w = await mountApp()
    await t(w, 'tab-assurance').trigger('click')
    expect(w.find('.assurance-stub').attributes('data-token')).toBe('tok-from-session')
  })

  it('ignores a token left in localStorage (it must never persist)', async () => {
    localStorage.setItem('kr0ki:token', 'tok-in-local-storage')
    const w = await mountApp()
    await t(w, 'tab-assurance').trigger('click')
    expect(w.find('.assurance-stub').attributes('data-token')).toBe('')
  })

  it('saving it in Setup keeps it in sessionStorage only and hands it to the view', async () => {
    const w = await mountApp()
    await t(w, 'tab-setup').trigger('click')
    const input = t(w, 'kr0ki-token')
    expect(input.attributes('type')).toBe('password')
    await input.setValue('  tok-new-secret  ')
    await w.find('form.setup__form').trigger('submit')
    await flushPromises()
    expect(sessionStorage.getItem('kr0ki:token')).toBe('tok-new-secret')
    expect(JSON.stringify({ ...localStorage })).not.toContain('tok-new-secret')
    await t(w, 'tab-assurance').trigger('click')
    expect(w.find('.assurance-stub').attributes('data-token')).toBe('tok-new-secret')
  })

  it('clearing it removes it from the session', async () => {
    sessionStorage.setItem('kr0ki:token', 'tok-old')
    const w = await mountApp()
    await t(w, 'tab-setup').trigger('click')
    await t(w, 'kr0ki-token').setValue('')
    await w.find('form.setup__form').trigger('submit')
    await flushPromises()
    expect(sessionStorage.getItem('kr0ki:token')).toBeNull()
  })

  it('can be revealed and hidden while typing', async () => {
    const w = await mountApp()
    await t(w, 'tab-setup').trigger('click')
    expect(t(w, 'kr0ki-token').attributes('type')).toBe('password')
    await t(w, 'toggle-token').trigger('click')
    expect(t(w, 'kr0ki-token').attributes('type')).toBe('text')
    await t(w, 'toggle-token').trigger('click')
    expect(t(w, 'kr0ki-token').attributes('type')).toBe('password')
  })

  it('tells the person it is session-only', async () => {
    const w = await mountApp()
    await t(w, 'tab-setup').trigger('click')
    expect(w.text()).toContain('this browser session only')
  })
})
