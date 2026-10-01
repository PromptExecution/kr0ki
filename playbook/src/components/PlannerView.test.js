import { describe, expect, it } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import PlannerView from './PlannerView.vue'

const catalog = {
  types: [
    { id: 'erd', name: 'ER diagram', syntax: 'plantuml', exampleId: 'e2', useCases: ['data model'], blurb: 'Tables and keys.', samplePrompt: 'Draw an ERD.' },
    { id: 'class', name: 'Class diagram', syntax: 'plantuml', exampleId: 'e3', useCases: ['data model', 'architecture'], blurb: 'Types.', samplePrompt: 'Draw classes.' },
  ],
}
const examples = [{ id: 'e2', format: 'plantuml' }, { id: 'e3', format: 'plantuml' }]
const StoryStub = { name: 'StoryB00k', props: { planner: Boolean, threadId: String, agentUrl: String }, template: '<div data-testid="story-stub" />' }
const mountView = (props = {}) =>
  mount(PlannerView, { props: { catalog, examples, plannerSession: 'planner-abc', agentUrl: 'http://agent.test:8789', ...props }, global: { stubs: { StoryB00k: StoryStub } } })

describe('PlannerView', () => {
  it('hosts the planner chat on the session thread and says how an MCP client can steer the page', () => {
    const w = mountView({ uiStatus: 'open' })
    expect(w.findComponent(StoryStub).props()).toMatchObject({ planner: true, threadId: 'planner-abc', agentUrl: 'http://agent.test:8789' })
    expect(w.find('[data-testid="planner-session"]').text()).toContain('planner-abc')
    expect(w.find('.planner-link').text()).toContain('page linked')
  })

  it('shows an empty state until the planner suggests something', () => {
    const w = mountView()
    expect(w.find('[data-testid="planner-picks"]').text()).toContain('Nothing suggested yet')
    expect(w.findAll('.pick-card')).toHaveLength(0)
  })

  it('lists the shortlist plus the selected type, marks the best fit and shows the planner note', () => {
    const w = mountView({ suggested: ['erd'], selectedTypeId: 'class', suggestNote: 'you described tables' })
    const cards = w.findAll('.pick-card')
    expect(cards.map((c) => c.attributes('data-type-id'))).toEqual(['erd', 'class'])
    expect(cards[1].classes()).toContain('primary')
    expect(cards[1].text()).toContain('best fit')
    expect(cards[0].text()).not.toContain('best fit')
    expect(w.text()).toContain('you described tables')
  })

  it('ignores ids the catalog does not know and does not duplicate a selected suggestion', () => {
    const w = mountView({ suggested: ['erd', 'nope'], selectedTypeId: 'erd' })
    expect(w.findAll('.pick-card')).toHaveLength(1)
  })

  it('offers Edit, Agent and Show in gallery on each pick', async () => {
    const w = mountView({ suggested: ['erd'] })
    await w.find('[data-testid="pick-edit"]').trigger('click')
    expect(w.emitted('open-in-editor')[0]).toEqual([{ id: 'e2', format: 'plantuml' }])
    await w.find('[data-testid="pick-agent"]').trigger('click')
    expect(w.emitted('agent-handoff')[0]).toEqual([{ typeId: 'erd', syntax: 'plantuml', prompt: 'Draw an ERD.' }])
    await w.find('[data-testid="pick-gallery"]').trigger('click')
    expect(w.emitted('show-in-gallery')[0]).toEqual(['erd'])
    await flushPromises()
  })

  it('disables Edit when the type has no fixture', () => {
    const w = mountView({ suggested: ['erd'], examples: [] })
    expect(w.find('[data-testid="pick-edit"]').attributes('disabled')).toBeDefined()
  })
})
