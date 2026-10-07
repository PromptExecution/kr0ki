// Save a diagram as a checkpoint in a project: one commit in the project's history holding the diagram file, so it can be
// listed, diffed and restored later from Projects -> History. The project store keeps whole-tree commits, so a checkpoint
// carries every other file of the project forward untouched.
import { sourceFilename } from './renderSource.js'

const slug = (s) => String(s || '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40)

/** `diagrams/<title>.<ext>` for a format. */
export function checkpointPath(format, title = 'diagram') {
  return `diagrams/${sourceFilename(format, slug(title) || 'diagram')}`
}

/**
 * @param {object} store  a project store (src/lib/projects.js)
 * @param {{projectId?: string, newProjectName?: string, path: string, source: string, message: string}} req
 * @returns {{project: object, commit: object|null, unchanged: boolean}}  `commit` is null when the file already had this content.
 */
export function saveCheckpoint(store, { projectId = '', newProjectName = '', path, source, message }) {
  if (!String(source ?? '').trim()) throw new Error('There is no diagram code to save')
  const project = projectId ? store.getProject(projectId) : store.createProject({ name: newProjectName })
  if (!project) throw new Error('That project no longer exists')
  const files = { ...store.checkout(project.id, project.head), [path]: String(source) }
  const commit = store.commit(project.id, { files, message: message || `Checkpoint ${path}` })
  return { project: store.getProject(project.id), commit, unchanged: commit === null }
}
