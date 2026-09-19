import { describe, expect, it } from 'vitest'
import {
  createRevisionGraph, activeNode, addPromptNode, addEditNode,
  checkoutNode, forkFrom, serialize, deserialize, ancestryOf,
} from '../revisionGraph.js'

describe('revisionGraph', () => {
  it('creates a root node and activates it', () => {
    const g = createRevisionGraph({ source: 'a -> b', format: 'd2' })
    expect(g.nodes).toHaveLength(1)
    expect(activeNode(g).source).toBe('a -> b')
    expect(activeNode(g).kind).toBe('root')
  })

  it('appends prompt nodes that mutate the diagram', () => {
    const g = createRevisionGraph({ source: 'a -> b' })
    addPromptNode(g, { prompt: 'add node c', source: 'a -> b\nb -> c', format: 'd2' })
    expect(activeNode(g).source).toContain('b -> c')
    expect(activeNode(g).prompt).toBe('add node c')
    expect(g.nodes).toHaveLength(2)
  })

  it('time travel restores an ancestor without losing children', () => {
    const g = createRevisionGraph({ source: 'v1' })
    const v2 = addPromptNode(g, { prompt: 'p1', source: 'v2', format: 'd2' })
    const v3 = addPromptNode(g, { prompt: 'p2', source: 'v3', format: 'd2' })
    checkoutNode(g, v2.id)
    expect(activeNode(g).source).toBe('v2')
    expect(g.nodes).toHaveLength(3)
    expect(v3.parentId).toBe(v2.id)
  })

  it('fork creates a branch from an earlier node', () => {
    const g = createRevisionGraph({ source: 'v1' })
    const v2 = addPromptNode(g, { prompt: 'p1', source: 'v2', format: 'd2' })
    addPromptNode(g, { prompt: 'p2', source: 'v3', format: 'd2' })
    const branch = forkFrom(g, v2.id, 'try alt')
    expect(branch.parentId).toBe(v2.id)
    expect(branch.source).toBe('v2')
    expect(activeNode(g).id).toBe(branch.id)
    const chain = ancestryOf(g, branch.id)
    expect(chain.map((n) => n.source)).toEqual(['v1', 'v2', 'v2'])
  })

  it('edit nodes record manual source changes', () => {
    const g = createRevisionGraph({ source: 'v1' })
    addEditNode(g, { source: 'v1 edited', format: 'd2' })
    expect(activeNode(g).kind).toBe('edit')
  })

  it('serializes and restores round-trip', () => {
    const g = createRevisionGraph({ source: 'a' })
    addPromptNode(g, { prompt: 'p', source: 'b', format: 'd2' })
    forkFrom(g, g.nodes[0].id)
    const json = serialize(g)
    const restored = deserialize(json)
    expect(restored.nodes).toHaveLength(3)
    expect(activeNode(restored).kind).toBe('fork')
  })

  it('rejects foreign payloads', () => {
    expect(() => deserialize({ version: 99, nodes: [] })).toThrow()
  })
})
