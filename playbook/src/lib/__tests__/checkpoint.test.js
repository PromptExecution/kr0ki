import { describe, expect, it } from 'vitest'
import { createProjectStore } from '../projects.js'
import { checkpointPath, saveCheckpoint } from '../checkpoint.js'

function mem() {
  const m = new Map()
  return { getItem: (k) => (m.has(k) ? JSON.parse(m.get(k)) : null), set: (k, v) => m.set(k, JSON.stringify(v)), remove: (k) => m.delete(k), getAllKeys: () => [...m.keys()] }
}

describe('checkpoints', () => {
  it('names the file from the title and format', () => {
    expect(checkpointPath('d2', 'My Big Diagram!')).toBe('diagrams/my-big-diagram.d2')
    expect(checkpointPath('graphviz')).toBe('diagrams/diagram.dot')
  })

  it('creates a project, commits the diagram, and it can be checked out again', () => {
    const store = createProjectStore({ storage: mem() })
    const r = saveCheckpoint(store, { newProjectName: 'Agents', path: 'diagrams/a.d2', source: 'a -> b', message: 'first' })
    expect(r.commit.message).toBe('first')
    expect(store.checkout(r.project.id, r.project.head)).toEqual({ 'diagrams/a.d2': 'a -> b' })
  })

  it('keeps other files, adds a new commit per change, and reports an unchanged save', () => {
    const store = createProjectStore({ storage: mem() })
    const p = store.createProject({ name: 'P' })
    store.commit(p.id, { files: { 'model.sysml': 'package P {}' }, message: 'init' })
    const a = saveCheckpoint(store, { projectId: p.id, path: 'diagrams/a.d2', source: 'a -> b', message: 'v1' })
    const b = saveCheckpoint(store, { projectId: p.id, path: 'diagrams/a.d2', source: 'a -> b\nb -> c', message: 'v2' })
    const same = saveCheckpoint(store, { projectId: p.id, path: 'diagrams/a.d2', source: 'a -> b\nb -> c', message: 'again' })
    expect(Object.keys(store.checkout(p.id, b.project.head)).sort()).toEqual(['diagrams/a.d2', 'model.sysml'])
    expect(same.unchanged).toBe(true)
    expect(store.log(p.id).map((c) => c.message)).toEqual(['v2', 'v1', 'init'])
    expect(store.checkout(p.id, a.commit.id)['diagrams/a.d2']).toBe('a -> b') // the earlier checkpoint is recoverable
  })

  it('refuses empty code and unknown projects', () => {
    const store = createProjectStore({ storage: mem() })
    expect(() => saveCheckpoint(store, { newProjectName: 'x', path: 'diagrams/a.d2', source: '  ', message: 'm' })).toThrow('no diagram code')
    expect(() => saveCheckpoint(store, { projectId: 'nope', path: 'diagrams/a.d2', source: 'a', message: 'm' })).toThrow('no longer exists')
  })
})
