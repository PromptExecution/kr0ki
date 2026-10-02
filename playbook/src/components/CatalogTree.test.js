import { describe, expect, it } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { Quasar } from 'quasar'
import CatalogTree from './CatalogTree.vue'

const catalog = {
  useCases: ['process flow', 'data model'],
  types: [
    { id: 'flowchart', name: 'Flowchart', syntax: 'd2', exampleId: 'e1', useCases: ['process flow'] },
    { id: 'erd', name: 'ER diagram', syntax: 'plantuml', exampleId: 'e2', useCases: ['data model', 'process flow'] },
  ],
}
const examples = [
  { id: 'e1', title: 'Flow', format: 'd2' },
  { id: 'e2', title: 'ER', format: 'plantuml' },
  { id: 'e9', title: 'Unreferenced fixture', format: 'graphviz' },
]
const mountTree = (props = {}) =>
  mount(CatalogTree, { props: { catalog, examples, ...props }, global: { plugins: [[Quasar, {}]] } })

describe('CatalogTree', () => {
  it('groups types by what they are for, expanded, with the other fixtures collapsed but reachable', async () => {
    const w = mountTree()
    await flushPromises()
    const text = w.text()
    expect(text).toContain('process flow (2)')
    expect(text).toContain('data model (1)')
    expect(text).toContain('Flowchart')
    expect(text).toContain('other fixtures (1)')
    expect(text).not.toContain('Unreferenced fixture') // collapsed until opened
    expect(w.findAll('.tree-leaf').length).toBe(3) // flowchart + erd under process flow, erd under data model
  })

  it('draws the expand arrow as an image, not as the Material Icons ligature text "play_arrow" (no icon font is bundled)', async () => {
    const w = mountTree()
    await flushPromises()
    expect(w.text()).not.toContain('play_arrow')
    const arrows = w.findAll('.q-tree__arrow')
    expect(arrows.length).toBeGreaterThan(0)
    for (const a of arrows) {
      expect(a.text()).toBe('')
      expect(a.find('img').attributes('src')).toMatch(/^data:image\/svg\+xml/)
    }
  })

  it('selecting a type emits its example and type id; selecting a group only toggles it', async () => {
    const w = mountTree()
    await flushPromises()
    await w.find('.tree-leaf[data-type-id="flowchart"]').trigger('click')
    expect(w.emitted('select')).toEqual([[{ exampleId: 'e1', typeId: 'flowchart' }]])
    await w.findAll('.tree-group')[0].trigger('click')
    expect(w.emitted('select')).toHaveLength(1)
  })

  it('opening "other fixtures" exposes unreferenced examples, which select without a type id', async () => {
    const w = mountTree()
    await flushPromises()
    await w.findAll('.tree-group').find((g) => g.text().startsWith('other fixtures')).trigger('click')
    await flushPromises()
    const leaf = w.findAll('.tree-leaf').find((l) => l.text().includes('Unreferenced fixture'))
    expect(leaf).toBeTruthy()
    await leaf.trigger('click')
    expect(w.emitted('select').at(-1)).toEqual([{ exampleId: 'e9', typeId: '' }])
  })

  it('marks the planner suggestions and renders compactly (no extra row padding classes)', async () => {
    const w = mountTree({ suggested: ['erd'] })
    await flushPromises()
    expect(w.findAll('.tree-leaf.suggested').length).toBe(2) // erd appears under two groups
    expect(w.findAll('.tree-star').length).toBe(2)
    expect(w.find('.q-tree').classes()).toContain('q-tree--dense')
  })

  it('shows a loading note while there is nothing to list, and falls back to the fixtures if the catalog never arrives', async () => {
    const empty = mountTree({ catalog: null, examples: [] })
    expect(empty.text()).toContain('Loading types')
    expect(empty.find('.q-tree').exists()).toBe(false)
    const fallback = mountTree({ catalog: null })
    await flushPromises()
    expect(fallback.text()).toContain('other fixtures (3)') // every example stays reachable without the catalog
  })
})
