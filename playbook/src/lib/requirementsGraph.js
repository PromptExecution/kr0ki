import { CODE_RE } from './sysmlText.js'

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
  const node = (id) => { if (!nodes.has(id)) nodes.set(id, { id, title: '', cost: null, attributions: [], tags: [], declaredIn: [], satisfiedBy: [], verifiedBy: [], allocatedTo: [], depictedIn: [] }); return nodes.get(id) }
  for (const d of declared) {
    const n = node(d.id)
    n.title = n.title || (d.doc || d.name || '').slice(0, 80)
    if (typeof d.attributes?.[COST] === 'number') n.cost = d.attributes[COST]
    for (const a of d.attributions || []) addAttribution(n, a.code, a.share)
    for (const t of d.tags || []) addTag(n, t.tag, t.basis)
    if (d.artifact && !n.declaredIn.includes(d.artifact)) n.declaredIn.push(d.artifact)
  }
  const edges = []
  for (const t of traces) {
    if (t.relation === 'declares') { const n = node(t.requirement); if (t.artifact && !n.declaredIn.includes(t.artifact)) n.declaredIn.push(t.artifact); continue }
    const n = node(t.requirement)
    const rel = normalize(t.relation)
    if (rel === 'attributed' && t.target) { addAttribution(n, t.target, t.share ?? 1); continue }
    if (rel === 'tagged' && t.target) { addTag(n, t.target, t.basis); continue }
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

// The same line from two sources (SysML metadata and a manual link) must not double-count.
function addAttribution(n, code, share) {
  const s = Number(share)
  if (!n.attributions.some((a) => a.code === code && a.share === s)) n.attributions.push({ code, share: Number.isFinite(s) ? s : 1 })
}
function addTag(n, tag, basis) {
  if (!n.tags.some((t) => t.tag === tag)) n.tags.push({ tag, basis: basis || 'judgement' })
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
    const codes = n.attributions.length ? n.attributions.map((a) => `${a.code} ${Math.round(a.share * 100)}%`).join(', ') : ''
    const label = [n.id, n.title, codes, cost && cost + sum].filter(Boolean).join('\n')
    const style = [gaps.has(n.id) ? 'style.stroke: "#f87171"' : '', r && !r.included ? 'style.opacity: 0.35' : ''].filter(Boolean)
    lines.push(`${q(n.id)}: ${q(label)}${style.length ? ` { ${style.join('; ')} }` : ''}`)
  }
  for (const e of graph.edges) lines.push(`${q(e.from)} -> ${q(e.to)}: ${q(e.kind)}`)
  return lines.join('\n') + '\n'
}

const EPS = 1e-9
const prefixes = (code) => code.split('/').map((_, i, a) => a.slice(0, i + 1).join('/'))

/**
 * Cost ATTRIBUTION summary (accounting-style codes, not amounts). Sums are in "requirement-equivalents" (a share of a requirement),
 * never currency. `include` = scenario set or null; `codebook` = Set of valid codes (or null to skip the unknown-code check).
 * @returns {{ byCode: {code,requirements:string[],share:number}[], byPrefix: {prefix,requirements:string[],share:number}[],
 *             unattributed:string[], overAllocated:{id,total:number}[], partlyAttributed:{id,total:number}[],
 *             invalidCodes:string[], unknownCodes:{code,requirements:string[]}[] }}
 */
export function attributionSummary(graph, { include = null, codebook = null } = {}) {
  const inScenario = (id) => include === null || include.has(id)
  const byCode = new Map(), byPrefix = new Map()
  const bump = (map, key, id, share) => { const e = map.get(key) || { requirements: new Set(), share: 0 }; e.requirements.add(id); e.share += share; map.set(key, e) }
  const unattributed = [], overAllocated = [], partlyAttributed = []
  const invalid = new Set(), unknown = new Map()
  for (const n of graph.nodes.filter((x) => inScenario(x.id))) {
    if (!n.attributions.length) { unattributed.push(n.id); continue }
    let total = 0
    for (const a of n.attributions) {
      total += a.share
      if (!CODE_RE.test(a.code)) { invalid.add(a.code); continue }
      bump(byCode, a.code, n.id, a.share)
      for (const p of prefixes(a.code)) bump(byPrefix, p, n.id, a.share)
      if (codebook && !codebook.has(a.code)) { const u = unknown.get(a.code) || new Set(); u.add(n.id); unknown.set(a.code, u) }
    }
    if (total > 1 + EPS) overAllocated.push({ id: n.id, total: round(total) })
    else if (total < 1 - EPS) partlyAttributed.push({ id: n.id, total: round(total) })
  }
  const rows = (map, key) => [...map].map(([k, v]) => ({ [key]: k, requirements: [...v.requirements].sort(), share: round(v.share) })).sort((a, b) => a[key].localeCompare(b[key]))
  return {
    byCode: rows(byCode, 'code'), byPrefix: rows(byPrefix, 'prefix'), unattributed, overAllocated, partlyAttributed,
    invalidCodes: [...invalid].sort(), unknownCodes: [...unknown].map(([code, r]) => ({ code, requirements: [...r].sort() })).sort((a, b) => a.code.localeCompare(b.code)),
  }
}
const round = (x) => Math.round(x * 1e6) / 1e6

/** Attribution rolled up the derivation tree: for each requirement, the code -> share over itself and everything derived from it (each once). */
export function rollUpAttribution(graph, include = null) {
  const kids = childrenOf(graph)
  const own = new Map(graph.nodes.map((n) => [n.id, n.attributions]))
  const inScenario = (id) => include === null || include.has(id)
  const out = {}
  for (const n of graph.nodes) {
    const seen = new Set([n.id]), stack = [n.id], codes = {}
    while (stack.length) {
      const id = stack.pop()
      if (inScenario(id)) for (const a of own.get(id) || []) codes[a.code] = round((codes[a.code] || 0) + a.share)
      for (const c of kids.get(id) || []) if (!seen.has(c)) { seen.add(c); stack.push(c) }
    }
    out[n.id] = codes
  }
  return out
}
