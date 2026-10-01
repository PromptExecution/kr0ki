// Local projects: SysML v2 textual files + diagram sources, saved as content-addressed commits with traceability to requirements.
// "Local storage is one way to save": this is the browser backend (Quasar's LocalStorage plugin); a server backend (the OMG
// Systems Modeling API via kr0ki's /model/projects/{id}/sync) is the other and shares the same commit shape.
//
//   project  = { id, name, head, createdAt, updatedAt }          (index entry + meta)
//   commit   = { id, parent, message, author, time, tree: {path: blobId}, changes: [{path,status}], traces: [...],
//                impact: { requirements: [...] } }               id = sha256 of the canonical commit body
//   blob     = file text, stored once per project under its sha256 (identical files across commits cost nothing)
//
// Limits (be honest): localStorage is synchronous and capped (~5 MB per origin, shared with the rest of the playbook), so
// `usage()` reports consumption and `commit()` throws QuotaError rather than half-writing. Export bundles move work elsewhere.
import { sha256 } from '@noble/hashes/sha2.js'
import { bytesToHex, utf8ToBytes } from '@noble/hashes/utils.js'
import { tracesOf } from './sysmlText.js'

export class QuotaError extends Error {
  constructor(message) { super(message); this.name = 'QuotaError' }
}
export class ProjectError extends Error {
  constructor(message) { super(message); this.name = 'ProjectError' }
}

export const BUNDLE_FORMAT = 'kr0ki-project/1'
const INDEX = 'kr0ki:projects'
const k = (id, ...rest) => ['kr0ki:p', id, ...rest].join(':')
const sha = (text) => bytesToHex(sha256(utf8ToBytes(text)))
const safePath = (p) => typeof p === 'string' && p.length > 0 && p.length < 200 && !p.startsWith('/') && !p.split('/').some((s) => s === '..' || s === '' || s === '.')

// stable stringify (sorted keys) so a commit's id never depends on key order
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
  if (value && typeof value === 'object') return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`
  return JSON.stringify(value)
}

export function fileKind(path) {
  if (/\.(sysml|kerml)$/i.test(path)) return 'sysml'
  if (/\.reqif(z)?$/i.test(path)) return 'reqif'
  return 'diagram'
}

/** @param {{ storage: object, now?: () => string, uuid?: () => string }} deps  `storage` is Quasar's LocalStorage (or anything with get/set/remove/getAllKeys). */
export function createProjectStore({ storage, now = () => new Date().toISOString(), uuid = () => globalThis.crypto?.randomUUID?.() || `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 10)}` }) {
  const read = (key, fallback = null) => { const v = storage.getItem(key); return v === null || v === undefined ? fallback : v }
  const write = (key, value) => {
    try { storage.set(key, value) } catch (err) {
      if (/quota/i.test(`${err?.name} ${err?.message}`)) throw new QuotaError('The browser storage is full. Export the project and delete old ones.')
      throw err
    }
  }
  const index = () => read(INDEX, [])
  const saveIndex = (list) => write(INDEX, list)
  const need = (id) => { const p = read(k(id, 'meta')); if (!p) throw new ProjectError(`unknown project ${id}`); return p }

  function createProject({ name }) {
    const clean = String(name ?? '').trim()
    if (!clean) throw new ProjectError('a project needs a name')
    const t = now()
    const project = { id: uuid(), name: clean.slice(0, 80), head: null, createdAt: t, updatedAt: t }
    write(k(project.id, 'meta'), project)
    saveIndex([...index(), project.id])
    return project
  }
  const getProject = (id) => read(k(id, 'meta'))
  const listProjects = () => index().map(getProject).filter(Boolean).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))

  function renameProject(id, name) {
    const p = need(id); const clean = String(name ?? '').trim()
    if (!clean) throw new ProjectError('a project needs a name')
    write(k(id, 'meta'), { ...p, name: clean.slice(0, 80), updatedAt: now() })
  }

  function deleteProject(id) {
    for (const key of storage.getAllKeys()) if (key.startsWith(k(id) + ':')) storage.remove(key)
    saveIndex(index().filter((x) => x !== id))
  }

  const getCommit = (id, commitId) => read(k(id, 'commit', commitId))
  const blob = (id, blobId) => read(k(id, 'blob', blobId))

  /** Files at a commit as {path: text}. `null`/undefined commit = the (empty) initial state. */
  function checkout(id, commitId) {
    if (!commitId) return {}
    const c = getCommit(id, commitId)
    if (!c) throw new ProjectError(`unknown commit ${commitId}`)
    return Object.fromEntries(Object.entries(c.tree).map(([path, b]) => [path, blob(id, b)]))
  }

  function diffTrees(a, b) {
    const out = []
    for (const path of new Set([...Object.keys(a), ...Object.keys(b)])) {
      if (!(path in a)) out.push({ path, status: 'added' })
      else if (!(path in b)) out.push({ path, status: 'removed' })
      else if (a[path] !== b[path]) out.push({ path, status: 'modified' })
    }
    return out.sort((x, y) => x.path.localeCompare(y.path))
  }

  /**
   * Save the working files as a new commit. `files` is the COMPLETE tree ({path: text}). Returns the commit, or null when nothing
   * changed. Traces (requirement links) are extracted from SysML text and merged with any `traces` supplied (e.g. a diagram that
   * depicts REQ-4: {requirement:'REQ-4', relation:'depicts', artifact:'diagrams/ctx.d2'}).
   */
  function commit(id, { files, message, author = 'local', traces = [] }) {
    const project = need(id)
    const msg = String(message ?? '').trim()
    if (!msg) throw new ProjectError('a commit needs a message')
    for (const path of Object.keys(files || {})) if (!safePath(path)) throw new ProjectError(`bad file path: ${path}`)
    const parent = project.head
    const before = parent ? getCommit(id, parent).tree : {}
    const tree = {}
    const newBlobs = []
    for (const [path, text] of Object.entries(files || {})) {
      const b = sha(String(text))
      tree[path] = b
      if (read(k(id, 'blob', b)) === null) newBlobs.push([b, String(text)])
    }
    const changes = diffTrees(before, tree)
    if (!changes.length) return null
    const extracted = Object.entries(files).filter(([p]) => fileKind(p) === 'sysml').flatMap(([p, t]) => tracesOf(p, String(t)))
    const all = dedupeTraces([...extracted, ...traces])
    const changedPaths = new Set(changes.map((c) => c.path))
    // Requirements affected by this commit: declared in a changed file, or linked from/to a changed artifact (now or before).
    const previous = parent ? getCommit(id, parent).traces : []
    const impacted = new Set()
    for (const t of [...all, ...previous]) {
      if (changedPaths.has(t.artifact)) impacted.add(t.requirement)
      if (t.target && changedPaths.has(t.target)) impacted.add(t.requirement)
    }
    const body = { parent, message: msg, author, time: now(), tree, changes, traces: all, impact: { requirements: [...impacted].sort() } }
    const commitId = sha(canonical(body))
    // write blobs and commit, then move head; if anything throws we roll back what we added
    const written = []
    try {
      for (const [b, text] of newBlobs) { write(k(id, 'blob', b), text); written.push(k(id, 'blob', b)) }
      write(k(id, 'commit', commitId), { id: commitId, ...body }); written.push(k(id, 'commit', commitId))
      write(k(id, 'meta'), { ...project, head: commitId, updatedAt: body.time })
    } catch (err) {
      for (const key of written) storage.remove(key)
      throw err
    }
    return { id: commitId, ...body }
  }

  function log(id, { limit = 50 } = {}) {
    const out = []
    for (let c = getCommit(id, need(id).head); c && out.length < limit; c = c.parent ? getCommit(id, c.parent) : null) out.push(c)
    return out
  }

  function diff(id, fromCommit, toCommit) {
    const a = fromCommit ? getCommit(id, fromCommit)?.tree : {}
    const b = toCommit ? getCommit(id, toCommit)?.tree : {}
    if (!a || !b) throw new ProjectError('unknown commit')
    return diffTrees(a, b)
  }

  /** The requirement graph of a commit (nodes = requirement ids, edges = trace links between requirements/artifacts). */
  function traceability(id, commitId = need(id).head) {
    const c = commitId ? getCommit(id, commitId) : null
    const links = c ? c.traces.filter((t) => t.relation !== 'declares') : []
    const requirements = new Set(c ? c.traces.map((t) => t.requirement) : [])
    return { requirements: [...requirements].sort(), links }
  }

  /** How the history of one requirement reads: every commit that touched it, newest first. */
  function requirementHistory(id, requirementId) {
    return log(id, { limit: 1000 }).filter((c) => c.impact.requirements.includes(requirementId)).map((c) => ({ commit: c.id, message: c.message, time: c.time, changes: c.changes }))
  }

  /** Unsaved work (files + manually added traces) survives a reload; cleared by passing null. */
  function saveDraft(id, draft) {
    need(id)
    if (draft === null) storage.remove(k(id, 'draft'))
    else write(k(id, 'draft'), { files: draft.files || {}, traces: draft.traces || [], selected: draft.selected || null })
  }
  const loadDraft = (id) => read(k(id, 'draft'))

  function exportProject(id) {
    const p = need(id)
    const commits = []
    for (let c = p.head ? getCommit(id, p.head) : null; c; c = c.parent ? getCommit(id, c.parent) : null) commits.push(c)
    const blobs = {}
    for (const c of commits) for (const b of Object.values(c.tree)) blobs[b] = blob(id, b)
    return { format: BUNDLE_FORMAT, project: { name: p.name, head: p.head, createdAt: p.createdAt }, commits: commits.reverse(), blobs }
  }

  /** Import a bundle as a NEW project. Every blob and commit id is re-derived and verified before anything is written. */
  function importProject(bundle) {
    if (bundle?.format !== BUNDLE_FORMAT) throw new ProjectError('not a kr0ki project bundle')
    const blobs = bundle.blobs || {}
    for (const [b, text] of Object.entries(blobs)) if (sha(String(text)) !== b) throw new ProjectError(`corrupt file content (${b.slice(0, 8)})`)
    let prev = null
    for (const c of bundle.commits || []) {
      const { id: cid, ...body } = c
      if (sha(canonical(body)) !== cid) throw new ProjectError(`corrupt commit ${String(cid).slice(0, 8)}`)
      if ((body.parent ?? null) !== prev) throw new ProjectError('commit history is not a single chain')
      for (const [path, b] of Object.entries(body.tree)) { if (!safePath(path)) throw new ProjectError(`bad file path: ${path}`); if (!(b in blobs)) throw new ProjectError(`missing file content for ${path}`) }
      prev = cid
    }
    if ((bundle.project?.head ?? null) !== prev) throw new ProjectError('bundle head does not match its history')
    const project = createProject({ name: `${bundle.project?.name || 'Imported project'}` })
    try {
      for (const [b, text] of Object.entries(blobs)) write(k(project.id, 'blob', b), text)
      for (const c of bundle.commits) write(k(project.id, 'commit', c.id), c)
      write(k(project.id, 'meta'), { ...project, head: prev, updatedAt: now() })
    } catch (err) { deleteProject(project.id); throw err }
    return getProject(project.id)
  }

  /** Approximate storage consumed by projects (UTF-16: 2 bytes per character), against a conservative 5 MB budget. */
  function usage(budgetBytes = 5 * 1024 * 1024) {
    let chars = 0
    for (const key of storage.getAllKeys()) if (key.startsWith('kr0ki:p:') || key === INDEX) chars += key.length + JSON.stringify(read(key) ?? '').length
    const bytes = chars * 2
    return { bytes, budgetBytes, fraction: bytes / budgetBytes }
  }

  return { saveDraft, loadDraft, createProject, getProject, listProjects, renameProject, deleteProject, commit, log, checkout, diff, getCommit, traceability, requirementHistory, exportProject, importProject, usage }
}

function dedupeTraces(list) {
  const seen = new Set()
  return list.filter((t) => { const key = `${t.requirement}|${t.relation}|${t.target}|${t.artifact}`; if (seen.has(key)) return false; seen.add(key); return true })
}
