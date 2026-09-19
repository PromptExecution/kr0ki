// Client-side revision DAG for StoryB00k diagram evolution.
//
// Every node is one state of the diagram: root (initial render from the agent),
// or a mutation (prompt-driven regeneration, editor edit). Nodes form a tree —
// forking from any node creates a branch; time travel just re-activates an
// ancestor. Persisted as JSON (jj-serializeable), rendered by vue-flow.
export const NODE_KINDS = { ROOT: 'root', PROMPT: 'prompt', EDIT: 'edit', FORK: 'fork' }

let idCounter = 0
function makeId(prefix) {
  idCounter += 1
  return `${prefix}-${Date.now().toString(36)}-${idCounter}`
}

export function createRevisionGraph({ source = '', format = 'd2', route = null, label = 'Initial diagram' } = {}) {
  const root = {
    id: makeId('rev'),
    parentId: null,
    kind: NODE_KINDS.ROOT,
    label,
    // What produced this state
    prompt: null,
    // The diagram state at this node
    source,
    format,
    route,
    // Agent feedback captured for regeneration context (clarifications etc.)
    notes: '',
    createdAt: new Date().toISOString(),
  }
  return { nodes: [root], activeId: root.id }
}

export function activeNode(graph) {
  return graph.nodes.find((n) => n.id === graph.activeId) || graph.nodes[0]
}

export function ancestryOf(graph, nodeId) {
  const byId = new Map(graph.nodes.map((n) => [n.id, n]))
  const chain = []
  let cur = byId.get(nodeId)
  while (cur) {
    chain.unshift(cur)
    cur = cur.parentId ? byId.get(cur.parentId) : null
  }
  return chain
}

/** Mutate the active node's diagram: append a prompt node on top of it. */
export function addPromptNode(graph, { prompt, source, format, route, notes = '' }) {
  const parent = activeNode(graph)
  const node = {
    id: makeId('rev'),
    parentId: parent.id,
    kind: NODE_KINDS.PROMPT,
    label: prompt.length > 48 ? `${prompt.slice(0, 48)}…` : prompt,
    prompt,
    source,
    format,
    route,
    notes,
    createdAt: new Date().toISOString(),
  }
  graph.nodes.push(node)
  graph.activeId = node.id
  return node
}

/** Time travel: make an existing node active again (no data change). */
export function checkoutNode(graph, nodeId) {
  if (!graph.nodes.some((n) => n.id === nodeId)) throw new Error(`unknown node ${nodeId}`)
  graph.activeId = nodeId
}

/** Fork: create a new branch from any node, copying its diagram state. */
export function forkFrom(graph, nodeId, label = 'Fork') {
  const base = graph.nodes.find((n) => n.id === nodeId)
  if (!base) throw new Error(`unknown node ${nodeId}`)
  const node = {
    ...base,
    id: makeId('rev'),
    parentId: base.id,
    kind: NODE_KINDS.FORK,
    label,
    createdAt: new Date().toISOString(),
  }
  graph.nodes.push(node)
  graph.activeId = node.id
  return node
}

/** Regenerate = a prompt node whose source starts as the parent's (agent fills it in). */
export function prepareRegeneration(graph, prompt) {
  const parent = activeNode(graph)
  return { parent, placeholder: addPromptNode(graph, { prompt, source: parent.source, format: parent.format, route: parent.route, notes: 'regenerating…' }) }
}

/** Edit on top of active node (from the EDIT button or manual editor change). */
export function addEditNode(graph, { source, format, route, label = 'Edited' }) {
  const parent = activeNode(graph)
  const node = {
    id: makeId('rev'),
    parentId: parent.id,
    kind: NODE_KINDS.EDIT,
    label,
    prompt: null,
    source,
    format,
    route,
    notes: '',
    createdAt: new Date().toISOString(),
  }
  graph.nodes.push(node)
  graph.activeId = node.id
  return node
}

/** JSON persistence shape (small, stable, jj-friendly). */
export function serialize(graph) {
  return JSON.stringify({ version: 1, activeId: graph.activeId, nodes: graph.nodes }, null, 2)
}

export function deserialize(json) {
  const data = typeof json === 'string' ? JSON.parse(json) : json
  if (data?.version !== 1 || !Array.isArray(data.nodes) || !data.nodes.length) {
    throw new Error('unrecognized revision-graph payload')
  }
  return { nodes: data.nodes, activeId: data.activeId || data.nodes[data.nodes.length - 1].id }
}
