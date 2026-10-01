import { beforeEach, describe, expect, it } from 'vitest'
import { Quasar, LocalStorage } from 'quasar'
import { createApp } from 'vue'
import { BUNDLE_FORMAT, ProjectError, QuotaError, createProjectStore, fileKind } from '../projects.js'

// Quasar's own LocalStorage plugin, installed the way main.js does it, over jsdom's localStorage.
function realQuasarStorage() {
  createApp({ render: () => null }).use(Quasar, { plugins: { LocalStorage } })
  return LocalStorage
}
// A minimal in-memory engine with the same surface, for quota and failure injection.
function memoryStorage({ limit = Infinity } = {}) {
  const m = new Map()
  return {
    getItem: (key) => (m.has(key) ? structuredClone(m.get(key)) : null),
    set(key, value) { const size = [...m].reduce((n, [a, b]) => n + a.length + JSON.stringify(b).length, 0) + key.length + JSON.stringify(value).length; if (size > limit) { const e = new Error('The quota has been exceeded'); e.name = 'QuotaExceededError'; throw e } m.set(key, structuredClone(value)) },
    remove: (key) => m.delete(key),
    getAllKeys: () => [...m.keys()],
    size: () => m.size,
  }
}
let n = 0
const deps = (storage) => ({ storage, now: () => `2026-10-01T10:00:${String(n++).padStart(2, '0')}Z`, uuid: () => `p${n++}` })

const REQ_V1 = "package P { requirement <'REQ-1'> mass : MassReq; part vehicle; satisfy mass by vehicle; }"
const REQ_V2 = "package P { requirement <'REQ-1'> mass : MassReq; requirement <'REQ-2'> range : RangeReq; part vehicle; satisfy mass by vehicle; }"

describe('with Quasar LocalStorage', () => {
  beforeEach(() => { localStorage.clear(); n = 0 })

  it('persists projects and commits through the real plugin and reads them back in a fresh store', () => {
    const a = createProjectStore(deps(realQuasarStorage()))
    const p = a.createProject({ name: 'Vehicle' })
    const c = a.commit(p.id, { files: { 'model/vehicle.sysml': REQ_V1 }, message: 'first' })
    const b = createProjectStore(deps(realQuasarStorage())) // e.g. after a page reload
    expect(b.listProjects().map((x) => x.name)).toEqual(['Vehicle'])
    expect(b.checkout(p.id, c.id)['model/vehicle.sysml']).toBe(REQ_V1)
    expect(Object.keys(localStorage).every((key) => key.startsWith('kr0ki:') || key.startsWith('__q'))).toBe(true)
  })
})

describe('commits', () => {
  let store, p
  beforeEach(() => { n = 0; store = createProjectStore(deps(memoryStorage())); p = store.createProject({ name: 'Demo' }) })

  it('records a content-addressed history with changes, and returns null when nothing changed', () => {
    const c1 = store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'ctx.d2': 'a -> b' }, message: 'initial' })
    expect(c1.parent).toBeNull()
    expect(c1.changes).toEqual([{ path: 'a.sysml', status: 'added' }, { path: 'ctx.d2', status: 'added' }])
    expect(store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'ctx.d2': 'a -> b' }, message: 'noop' })).toBeNull()
    const c2 = store.commit(p.id, { files: { 'a.sysml': REQ_V2 }, message: 'add range; drop diagram' })
    expect(c2.parent).toBe(c1.id)
    expect(c2.changes).toEqual([{ path: 'a.sysml', status: 'modified' }, { path: 'ctx.d2', status: 'removed' }])
    expect(store.log(p.id).map((c) => c.message)).toEqual(['add range; drop diagram', 'initial'])
    expect(store.checkout(p.id, c1.id)['ctx.d2']).toBe('a -> b')
    expect(store.diff(p.id, c1.id, c2.id)).toEqual(c2.changes)
  })

  it('ids are deterministic and identical files are stored once', () => {
    const s2 = createProjectStore(deps(memoryStorage())); const p2 = s2.createProject({ name: 'Demo' })
    const files = { 'a.sysml': REQ_V1 }
    const x = store.commit(p.id, { files, message: 'm', author: 'x' })
    expect(x.id).toMatch(/^[0-9a-f]{64}$/)
    store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'copy.sysml': REQ_V1 }, message: 'same text twice' })
    const head = store.getCommit(p.id, store.getProject(p.id).head)
    expect(Object.keys(head.tree)).toEqual(['a.sysml', 'copy.sysml'])
    expect(new Set(Object.values(head.tree)).size).toBe(1) // two paths, one blob
    expect(Object.keys(store.exportProject(p.id).blobs)).toHaveLength(1)
    expect(s2.commit(p2.id, { files, message: 'm', author: 'x' }).tree).toEqual(x.tree)
  })

  it('refuses empty messages, unsafe paths and unknown projects', () => {
    expect(() => store.commit(p.id, { files: { 'a.sysml': 'x' }, message: '  ' })).toThrow(ProjectError)
    for (const bad of ['../x', '/abs', 'a//b', './a', '', 'a/../b']) expect(() => store.commit(p.id, { files: { [bad]: 'x' }, message: 'm' }), bad).toThrow(/bad file path/)
    expect(() => store.commit('nope', { files: {}, message: 'm' })).toThrow(/unknown project/)
    expect(() => store.createProject({ name: '  ' })).toThrow(ProjectError)
  })
})

describe('traceability of changes to requirements', () => {
  let store, p
  beforeEach(() => { n = 0; store = createProjectStore(deps(memoryStorage())); p = store.createProject({ name: 'Demo' }) })

  it('extracts requirement links from SysML text and reports which requirements a commit affects', () => {
    store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'other.d2': 'x -> y' }, message: 'initial' })
    const c2 = store.commit(p.id, { files: { 'a.sysml': REQ_V2, 'other.d2': 'x -> y' }, message: 'add range' })
    expect(c2.impact.requirements).toEqual(['REQ-1', 'REQ-2']) // the whole file changed: everything it declares or links
    const c3 = store.commit(p.id, { files: { 'a.sysml': REQ_V2, 'other.d2': 'x -> z' }, message: 'tweak unrelated diagram' })
    expect(c3.impact.requirements).toEqual([]) // an unrelated diagram touches no requirement
    const g = store.traceability(p.id)
    expect(g.requirements).toEqual(['REQ-1', 'REQ-2'])
    expect(g.links).toContainEqual({ requirement: 'REQ-1', relation: 'satisfy', target: 'vehicle', artifact: 'a.sysml' })
  })

  it('links a diagram to a requirement it depicts, so editing the diagram flags that requirement', () => {
    const depicts = [{ requirement: 'REQ-1', relation: 'depicts', target: null, artifact: 'ctx.d2' }]
    store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'ctx.d2': 'a -> b' }, message: 'initial', traces: depicts })
    const c = store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'ctx.d2': 'a -> c' }, message: 'rework diagram', traces: depicts })
    expect(c.impact.requirements).toEqual(['REQ-1'])
    expect(store.requirementHistory(p.id, 'REQ-1').map((h) => h.message)).toEqual(['rework diagram', 'initial'])
    expect(store.requirementHistory(p.id, 'REQ-9')).toEqual([])
  })

  it('a removed file still flags the requirements it carried', () => {
    store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'b.sysml': 'package Q {}' }, message: 'initial' })
    const c = store.commit(p.id, { files: { 'b.sysml': 'package Q {}' }, message: 'delete the requirements file' })
    expect(c.impact.requirements).toEqual(['REQ-1'])
  })
})

describe('export / import and limits', () => {
  let store, p
  beforeEach(() => { n = 0; store = createProjectStore(deps(memoryStorage())); p = store.createProject({ name: 'Demo' }) })

  it('round-trips a project as a bundle into a new project with the same history', () => {
    store.commit(p.id, { files: { 'a.sysml': REQ_V1 }, message: 'one' })
    store.commit(p.id, { files: { 'a.sysml': REQ_V2 }, message: 'two' })
    const bundle = JSON.parse(JSON.stringify(store.exportProject(p.id)))
    expect(bundle.format).toBe(BUNDLE_FORMAT)
    const other = createProjectStore(deps(memoryStorage()))
    const q = other.importProject(bundle)
    expect(other.log(q.id).map((c) => c.message)).toEqual(['two', 'one'])
    expect(other.checkout(q.id, q.head)['a.sysml']).toBe(REQ_V2)
    expect(q.head).toBe(store.getProject(p.id).head) // identical content => identical commit ids
  })

  it('rejects tampered bundles before writing anything', () => {
    store.commit(p.id, { files: { 'a.sysml': REQ_V1 }, message: 'one' })
    const good = JSON.parse(JSON.stringify(store.exportProject(p.id)))
    const fresh = () => createProjectStore(deps(memoryStorage()))
    const tamperBlob = structuredClone(good); tamperBlob.blobs[Object.keys(tamperBlob.blobs)[0]] = 'evil'
    const tamperCommit = structuredClone(good); tamperCommit.commits[0].message = 'edited'
    const dropBlob = structuredClone(good); delete dropBlob.blobs[Object.keys(dropBlob.blobs)[0]]
    const wrongHead = structuredClone(good); wrongHead.project.head = 'f'.repeat(64)
    const traversal = structuredClone(good); traversal.commits[0].tree['../x'] = Object.values(traversal.commits[0].tree)[0]
    for (const [name, bad, re] of [['blob', tamperBlob, /corrupt file/], ['commit', tamperCommit, /corrupt commit/], ['missing', dropBlob, /corrupt|missing/], ['head', wrongHead, /head/], ['path', traversal, /corrupt commit|bad file path/], ['format', { format: 'x' }, /not a kr0ki/]]) {
      const s = fresh()
      expect(() => s.importProject(bad), name).toThrow(re)
      expect(s.listProjects(), `${name} left a project behind`).toEqual([])
    }
  })

  it('rolls back completely when the storage fills up, and says why', () => {
    const tight = memoryStorage({ limit: 1500 }); const s = createProjectStore(deps(tight)); const q = s.createProject({ name: 'Tight' })
    const before = tight.size()
    expect(() => s.commit(q.id, { files: { 'big.sysml': 'x'.repeat(4000) }, message: 'too big' })).toThrow(QuotaError)
    expect(tight.size()).toBe(before) // no orphan blobs or commits
    expect(s.getProject(q.id).head).toBeNull()
  })

  it('removes the file contents it already wrote when the commit record itself cannot be stored', () => {
    const m = new Map()
    const flaky = {
      getItem: (key) => (m.has(key) ? structuredClone(m.get(key)) : null),
      set(key, value) { if (key.includes(':commit:')) { const e = new Error('quota'); e.name = 'QuotaExceededError'; throw e } m.set(key, structuredClone(value)) },
      remove: (key) => m.delete(key), getAllKeys: () => [...m.keys()],
    }
    const s = createProjectStore(deps(flaky)); const q = s.createProject({ name: 'Flaky' })
    expect(() => s.commit(q.id, { files: { 'a.sysml': REQ_V1 }, message: 'm' })).toThrow(QuotaError)
    expect(m.size).toBe(2) // only the project index and meta remain: no orphaned blob
    expect(s.getProject(q.id).head).toBeNull()
  })

  it('rejects a bundle whose commits are out of order or not one chain', () => {
    store.commit(p.id, { files: { 'a.sysml': REQ_V1 }, message: 'one' })
    store.commit(p.id, { files: { 'a.sysml': REQ_V2 }, message: 'two' })
    const good = JSON.parse(JSON.stringify(store.exportProject(p.id)))
    const reordered = structuredClone(good); reordered.commits.reverse()
    const fresh = createProjectStore(deps(memoryStorage()))
    expect(() => fresh.importProject(reordered)).toThrow(/single chain/)
    const truncated = structuredClone(good); truncated.commits.shift() // second commit now has a parent that is not in the bundle
    expect(() => fresh.importProject(truncated)).toThrow(/single chain/)
    expect(fresh.listProjects()).toEqual([])
  })

  it('flags a requirement when a file it points to (the trace target) changes, even if the declaring file did not', () => {
    const link = [{ requirement: 'REQ-1', relation: 'implements', target: 'src/code.rs', artifact: 'a.sysml' }]
    store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'src/code.rs': 'fn a() {}' }, message: 'initial', traces: link })
    const c = store.commit(p.id, { files: { 'a.sysml': REQ_V1, 'src/code.rs': 'fn b() {}' }, message: 'change the code', traces: link })
    expect(c.changes.map((x) => x.path)).toEqual(['src/code.rs'])
    expect(c.impact.requirements).toEqual(['REQ-1'])
  })

  it('reports usage and deleting a project frees everything it stored', () => {
    const st = memoryStorage(); const s = createProjectStore(deps(st)); const q = s.createProject({ name: 'U' })
    s.commit(q.id, { files: { 'a.sysml': REQ_V1 }, message: 'm' })
    expect(s.usage().bytes).toBeGreaterThan(0)
    s.deleteProject(q.id)
    expect(s.listProjects()).toEqual([]); expect(st.getAllKeys()).toEqual(['kr0ki:projects'])
  })

  it('keeps an unsaved draft across reloads, and deleting the project removes it', () => {
    const st = memoryStorage(); const s = createProjectStore(deps(st)); const q = s.createProject({ name: 'D' })
    expect(s.loadDraft(q.id)).toBeNull()
    s.saveDraft(q.id, { files: { 'a.sysml': 'wip' }, traces: [{ requirement: 'R', relation: 'depicts', target: null, artifact: 'd.d2' }], selected: 'a.sysml' })
    expect(createProjectStore(deps(st)).loadDraft(q.id)).toEqual({ files: { 'a.sysml': 'wip' }, traces: [{ requirement: 'R', relation: 'depicts', target: null, artifact: 'd.d2' }], selected: 'a.sysml' })
    s.saveDraft(q.id, null); expect(s.loadDraft(q.id)).toBeNull()
    s.saveDraft(q.id, { files: {} }); s.deleteProject(q.id)
    expect(st.getAllKeys()).toEqual(['kr0ki:projects'])
  })

  it('classifies files by kind', () => {
    expect([fileKind('a.sysml'), fileKind('x/b.kerml'), fileKind('r.reqif'), fileKind('r.reqifz'), fileKind('d.d2')]).toEqual(['sysml', 'sysml', 'reqif', 'reqif', 'diagram'])
  })
})
