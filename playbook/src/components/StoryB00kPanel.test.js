import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import StoryB00kPanel from './StoryB00kPanel.vue'

describe('StoryB00kPanel', () => {
  it('labels query data distinctly from narration', () => {
    expect(mount(StoryB00kPanel, { props: { panel: { kind: 'query-result', content: '[]' } } }).text()).toContain('Model data')
    const narration = mount(StoryB00kPanel, { props: { panel: { kind: 'narration', content: 'One engine.' } } })
    expect(narration.text()).toContain('Agent narration')
    expect(narration.text()).not.toContain('Model data')
  })

  it('renders a diagram and its source together', () => {
    const panel = mount(StoryB00kPanel, { props: { panel: { kind: 'render', content: '<svg><title>x</title></svg>', source: { source: 'a -> b', format: 'd2' } } } })
    expect(panel.find('[data-testid="rendered-output"]').exists()).toBe(true)
    expect(panel.find('[data-testid="source-text"]').text()).toContain('a -> b')
  })
})
