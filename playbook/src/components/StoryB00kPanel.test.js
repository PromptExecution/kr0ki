import { describe, expect, it } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import StoryB00kPanel from './StoryB00kPanel.vue'
import StoryB00k from './StoryB00k.vue'

describe('StoryB00kPanel', () => {
  it('labels query data distinctly from narration', () => {
    expect(mount(StoryB00kPanel, { props: { panel: { kind: 'query-result', content: '[]' } } }).text()).toContain('Model data')
    const narration = mount(StoryB00kPanel, { props: { panel: { kind: 'narration', content: 'One engine.' } } })
    expect(narration.text()).toContain('Agent narration')
    expect(narration.text()).not.toContain('Model data')
  })

  it('renders a diagram and its source together', () => {
    const panel = mount(StoryB00kPanel, { props: { panel: { kind: 'render', content: '<svg><title>x</title></svg>', source: { text: 'a -> b', format: 'd2' } } } })
    expect(panel.find('[data-testid="rendered-output"]').exists()).toBe(true)
    expect(panel.find('[data-testid="source-text"]').text()).toContain('a -> b')
  })

  it('uses an image element for binary render output', () => {
    const panel = mount(StoryB00kPanel, { props: { panel: { kind: 'render', content: '', imageDataUrl: 'data:image/png;base64,AA==' } } })
    expect(panel.find('[data-testid="rendered-image"]').attributes('src')).toContain('image/png')
  })

  it('keeps the chat controls usable before the first agent event', async () => {
    const panel = mount(StoryB00k)
    await flushPromises()
    // The composer is a textarea now; typing is always allowed, and Send is
    // gated on non-empty input rather than on run status alone.
    expect(panel.find('textarea').attributes('disabled')).toBeUndefined()
    const send = panel.findAll('button').find(b => b.text() === 'Send')
    expect(send.attributes('disabled')).toBeDefined()
    expect(panel.findAll('.storyb00k-panel')).toHaveLength(0)
  })

  it('offers EDIT on a render panel that carries source+format up to the app', async () => {
    const StoryB00kPanel = (await import('./StoryB00kPanel.vue')).default
    const wrapper = mount(StoryB00kPanel, {
      props: { panel: { kind: 'render', content: '<svg/>', source: { text: 'a -> b', format: 'd2' } } },
    })
    const edit = wrapper.find('[data-testid="edit-in-editor"]')
    expect(edit.exists()).toBe(true)
    await edit.trigger('click')
    expect(wrapper.emitted('edit')).toEqual([[{ source: 'a -> b', format: 'd2' }]])
  })

  it('renders no EDIT button when the panel has no source', async () => {
    const StoryB00kPanel = (await import('./StoryB00kPanel.vue')).default
    const wrapper = mount(StoryB00kPanel, { props: { panel: { kind: 'render', content: '<svg/>' } } })
    expect(wrapper.find('[data-testid="edit-in-editor"]').exists()).toBe(false)
  })
})
