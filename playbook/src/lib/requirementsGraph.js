// The graph of requirements in a project commit, with coverage gaps and a cost roll-up under scenarios.
//
//   node  = a declared requirement  { id, title, cost (own, from `attribute cost = N`), declaredIn[] }
//   edge  = a trace between requirements {from, to, kind}; `derive` means `from` is derived from `to` (child -> parent)
//   cost model (local, deliberately simple): rollup(r) = own(r) + sum of own(d) over every INCLUDED requirement d derived
//   (transitively) from r, each counted once. A scenario is the set of included requirements; `total` sums own cost over it.
// Server side the same question is a SPARQL property path + SUM over the oxigraph store (PLAN-KR0KI-008 WP7).

const COST = 'cost'
const REQUIREMENT_LINKS = new Set(['derive', 'derives', 'refine', 'refines', 'satisfy', 'satisfies', 'verify', 'verifies', 'allocate', 'trace', 'traces', 'requires', 'contains', 'implements', 'depicts'])

/**
 * @param {{requirement:string, relation:string, target:string|null, artifact:string}[]} traces  commit traces
 * @param {{id:string, doc?:string, name?:string, attributes?:Record<string,number>, artifact?:string}[]} declared
 */
export function buildRequirementGraph(traces, declared = []) {
  const nodes = new Map()
  const node = (id) => { if (!nodes.has(id)) nodes.set(id, { id, title: '', cost: null, declaredIn: [], satisfiedBy: [], verifiedBy: [], allocatedTo: [], depictedIn: [] }); return nodes.get(id) }
  for (const d of declared) {
    const n = node(d.id)
    n.title = n.title || (d.doc || d.name || '').slice(0, 80)
    if (typeof d.attributes?.[COST] === 'number') n.cost = d.attributes[COST]
    if (d.artifact && !n.declaredIn.includes(d.artifact)) n.declaredIn.push(d.artifact)
  }
  const edges = []
  for (const t of traces) {
    if (t.relation === 'declares') { const n = node(t.requirement); if (t.artifact && !n.declaredIn.includes(t.artifact)) n.declaredIn.push(t.artifact); continue }
    const n = node(t.requirement)
    const rel = normalize(t.relation)
    if (rel === 'satisfy' && t.target) n.satisfiedBy.push(t.target)
    else if (rel === 'verify') n.verifiedBy.push(t.target || t.artifact)
    else if (rel === 'allocate' && t.target) n.allocatedTo.push(t.target)
    else if (rel === 'depicts') n.depictedIn.push(t.artifact)
    // an edge between two requirements: only when the target is itself a requirement we know about or a derive/refine/trace link
    if (t.target && ['derive', 'refine', 'trace', 'requires', 'contains', 'implements'].includes(rel)) {
      node(t.target)
      edges.push({ from: t.requirement, to: t.target, kind: rel })
    }
  }
  return { nodes: [...nodes.values()].sort((a, b) => a.id.localeCompare(b.id, undefined, { numeric: true })), edges: dedupe(edges) }
}

const normalize = (rel) => ({ derives: 'derive', refines: 'refine', satisfies: 'satisfy', verifies: 'verify', traces: 'trace' }[rel] || rel)
const dedupe = (edges) => { const seen = new Set(); return edges.filter((e) => { const k = `${e.from}>${e.to}>${e.kind}`; return seen.has(k) ? false : (seen.add(k), true) }) }

/** Ids declared in more than one file: they merge into one node, which is almost always a mistake worth telling the user about. */
export function duplicateIds(graph) {
  return graph.nodes.filter((n) => n.declaredIn.length > 1).map((n) => ({ id: n.id, files: [...n.declaredIn] }))
}

/** Requirements that have no satisfy and/or no verify link: the coverage gaps a non-expert should see first. */
export function coverageGaps(graph) {
  return graph.nodes
    .map((n) => ({ id: n.id, title: n.title, unsatisfied: n.satisfiedBy.length === 0, unverified: n.verifiedBy.length === 0, hasCost: n.cost !== null }))
    .filter((g) => g.unsatisfied || g.unverified)
}

/** Children of each requirement: those derived (child --derive--> parent) from it. */
function childrenOf(graph) {
  const kids = new Map(graph.nodes.map((n) => [n.id, []]))
  for (const e of graph.edges) if (e.kind === 'derive' && kids.has(e.to) && kids.has(e.from)) kids.get(e.to).push(e.from)
  return kids
}

/**
 * Cost roll-up. `include` = Set of requirement ids in the scenario, or null for all. Requirements with no cost count as 0 and are
 * listed in `unpriced` so a total is never silently too low.
 * @returns {{ total:number, perRequirement: Record<string,{own:number, rollup:number, included:boolean}>, unpriced:string[] }}
 */
export function rollUpCosts(graph, include = null) {
  const kids = childrenOf(graph)
  const own = new Map(graph.nodes.map((n) => [n.id, n.cost ?? 0]))
  const inScenario = (id) => include === null || include.has(id)
  const per = {}
  for (const n of graph.nodes) {
    const seen = new Set([n.id])
    const stack = [...kids.get(n.id)]
    let sum = inScenario(n.id) ? own.get(n.id) : 0
    while (stack.length) {
      const id = stack.pop()
      if (seen.has(id)) continue // cycle-safe, and a shared descendant counts once
      seen.add(id)
      if (inScenario(id)) sum += own.get(id)
      stack.push(...kids.get(id))
    }
    per[n.id] = { own: own.get(n.id), rollup: sum, included: inScenario(n.id) }
  }
  const total = graph.nodes.filter((n) => inScenario(n.id)).reduce((s, n) => s + own.get(n.id), 0)
  return { total, perRequirement: per, unpriced: graph.nodes.filter((n) => inScenario(n.id) && n.cost === null).map((n) => n.id) }
}

// a D2 double-quoted string: escape backslashes and quotes first, then turn real newlines into the \n escape
// (`$` starts a D2 substitution even inside double quotes, so a literal one is written \$)
const q = (s) => `"${String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\$/g, '\\$').replace(/\r?\n/g, '\\n')}"`

/** D2 source drawing the requirement graph; label = id, short title and cost roll-up; red outline = coverage gap. Rendered via /render/d2. */
export function toD2(graph, rollup = null) {
  const gaps = new Set(coverageGaps(graph).map((g) => g.id))
  const lines = ['direction: down']
  for (const n of graph.nodes) {
    const r = rollup?.perRequirement[n.id]
    const cost = n.cost !== null ? `cost ${n.cost}` : ''
    const sum = r && r.rollup !== r.own ? ` (Σ ${r.rollup})` : ''
    const label = [n.id, n.title, cost && cost + sum].filter(Boolean).join('\n')
    const style = [gaps.has(n.id) ? 'style.stroke: "#f87171"' : '', r && !r.included ? 'style.opacity: 0.35' : ''].filter(Boolean)
    lines.push(`${q(n.id)}: ${q(label)}${style.length ? ` { ${style.join('; ')} }` : ''}`)
  }
  for (const e of graph.edges) lines.push(`${q(e.from)} -> ${q(e.to)}: ${q(e.kind)}`)
  return lines.join('\n') + '\n'
}
