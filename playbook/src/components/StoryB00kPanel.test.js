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
    expect(panel.find('input').attributes('disabled')).toBeUndefined()
    expect(panel.find('button').attributes('disabled')).toBeDefined()
    expect(panel.findAll('.storyb00k-panel')).toHaveLength(0)
  })
})
