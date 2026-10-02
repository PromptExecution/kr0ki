// A light scanner for SysML v2 *textual notation*: pulls out requirement declarations and the relationships that matter for
// traceability. It is a heuristic text scanner, NOT a parser: it does not validate, resolve names or understand scoping. The
// server-side `sysml-v2-parser` is the authority; this exists so the browser can trace requirements offline (saving/committing
// local projects) without a server round trip.
//
// Recognised (inside any package/part/etc.; comments are ignored except `doc`):
//   requirement def Name { doc /* text */ ... }            -> definition
//   requirement <'ID'> name : Type { doc /* text */ ... }   -> usage (short name = the requirement's identifier)
//   satisfy [requirement] R by X;     verify [requirement] R;     refine R by X;     derive R from|by X;     allocate R to X;
//   requirement name :> base            (specialization = derivation)

const KINDS = ['satisfy', 'verify', 'refine', 'derive', 'allocate']

// Strip line and block comments, but keep the text of `doc` blocks by hoisting each one to a numbered marker first.
function withDocs(text) {
  const docs = []
  const hoisted = text.replace(/\bdoc\b\s*(?:<[^>]*>\s*)?\/\*([\s\S]*?)\*\//g, (_m, body) => {
    docs.push(body.replace(/^\s*\*+/gm, '').replace(/\s+/g, ' ').trim())
    return ` doc__${docs.length - 1}__ `
  })
  const stripped = hoisted.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/\/\/[^\n]*/g, ' ')
  return { text: stripped, docs }
}

const ID = String.raw`(?:'[^']+'|[A-Za-z_][\w]*)`
const unquote = (s) => (s && s.startsWith("'") ? s.slice(1, -1) : s)
const QNAME = String.raw`(?:${ID}(?:::${ID})*(?:\.${ID})*)`

/**
 * @returns {{definitions: {name:string, shortName:string|null, doc:string}[],
 *            usages: {name:string, shortName:string|null, type:string|null, general:string|null, doc:string}[],
 *            relations: {kind:string, requirement:string, target:string|null}[]}}
 */
// The brace-balanced `{ ... }` body that follows `index` (a declaration may end with `;` instead, meaning no body).
function bodyAfter(text, index) {
  const open = /^[^;{]*\{/.exec(text.slice(index, index + 400))
  if (!open) return ''
  let depth = 0
  for (let i = index + open[0].length - 1; i < text.length; i++) {
    if (text[i] === '{') depth++
    else if (text[i] === '}' && --depth === 0) return text.slice(index + open[0].length, i)
  }
  return text.slice(index + open[0].length) // unbalanced: take the rest rather than guess
}

// `attribute cost : Real = 120.5 [USD];` -> { cost: 120.5 }. Numeric literals only (no expressions): an honest, checkable subset.
// Cost ATTRIBUTION, not cost: an accounting-style code the requirement is charged to, written as SysML v2 metadata:
//   @CostAttribution { code = 'CC-4410/WBS-2.3'; share = 0.6; }      (also `metadata CostAttribution { ... }`)
// `code` is a hierarchical code (cost centre / work breakdown / activity); `share` is the fraction of the requirement attributed to it
// (default 1). Only metadata that sits directly in the requirement's own body counts (nested requirements keep their own).
export const CODE_RE = /^[A-Za-z0-9]+(?:[-_.][A-Za-z0-9]+)*(?:\/[A-Za-z0-9]+(?:[-_.][A-Za-z0-9]+)*)*$/

/** Split a body into its own text (nested blocks removed) and its nested blocks with the text that introduced each. */
function topLevel(body) {
  let depth = 0, plain = '', head = '', start = -1
  const blocks = []
  for (let i = 0; i < body.length; i++) {
    const ch = body[i]
    if (ch === '{') { if (depth++ === 0) { start = i + 1; head = plain.slice(-120) } }
    else if (ch === '}' && depth > 0) { if (--depth === 0) { blocks.push({ head, content: body.slice(start, i) }); plain += ' ' } }
    else if (depth === 0) plain += ch
  }
  return { plain, blocks }
}

// Compliance tags: `@ComplianceTag { tag = 'iso42001:A.6.2.6'; basis = 'judgement'; }` -> { tag, basis } (basis: official | judgement).
function complianceTags(blocks) {
  const out = []
  for (const b of blocks) {
    if (!/(?:@|\bmetadata\s+)ComplianceTag\s*$/.test(b.head.trimEnd())) continue
    const tag = new RegExp(String.raw`\btag\s*=\s*(?:'([^']*)'|"([^"]*)")\s*;`).exec(b.content)
    const basis = new RegExp(String.raw`\bbasis\s*=\s*(?:'([^']*)'|"([^"]*)")\s*;`).exec(b.content)
    if (tag) out.push({ tag: (tag[1] ?? tag[2]).trim(), basis: basis ? (basis[1] ?? basis[2]).trim() : 'judgement' })
  }
  return out
}

function costAttributions(blocks) {
  const out = []
  for (const b of blocks) {
    if (!/(?:@|\bmetadata\s+)CostAttribution\s*$/.test(b.head.trimEnd())) continue
    const code = new RegExp(String.raw`\bcode\s*=\s*(?:'([^']*)'|"([^"]*)")\s*;`).exec(b.content)
    const share = /\bshare\s*=\s*(-?\d+(?:\.\d+)?)\s*;/.exec(b.content)
    if (code) out.push({ code: (code[1] ?? code[2]).trim(), share: share ? Number(share[1]) : 1 })
  }
  return out
}

// Numeric literals only (no expressions) from the body's own text: an honest, checkable subset. Kept for old projects (`attribute cost = N;`).
function numericAttributes(body) {
  const out = {}
  const re = new RegExp(String.raw`\battribute\s+(${ID})(?:\s*:\s*${QNAME})?\s*=\s*(-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)\s*(?:\[[^\]]*\])?\s*;`, 'g')
  for (let m; (m = re.exec(body)); ) out[unquote(m[1])] = Number(m[2])
  return out
}

export function scanRequirements(source) {
  const { text, docs } = withDocs(String(source ?? ''))
  const docAfter = (index) => {
    // the first doc marker within this declaration's body (before the next requirement/part/package keyword at the same text run)
    const rest = text.slice(index, index + 1200)
    const m = /^[^;{]*\{\s*doc__(\d+)__/.exec(rest)
    return m ? docs[Number(m[1])] : ''
  }
  const definitions = []
  const usages = []
  const defRe = new RegExp(String.raw`\brequirement\s+def\s+(?:<\s*(${ID})\s*>\s*)?(${ID})`, 'g')
  for (let m; (m = defRe.exec(text)); ) {
    definitions.push({ name: unquote(m[2]), shortName: unquote(m[1]) ?? null, doc: docAfter(m.index + m[0].length), attributes: numericAttributes(topLevel(bodyAfter(text, m.index + m[0].length)).plain), attributions: costAttributions(topLevel(bodyAfter(text, m.index + m[0].length)).blocks), tags: complianceTags(topLevel(bodyAfter(text, m.index + m[0].length)).blocks) })
  }
  // usage: `requirement [<'id'>] name [: Type] [:> general]` that is not `requirement def`, `satisfy requirement` or `verify requirement`
  const useRe = new RegExp(String.raw`(?<!\b(?:satisfy|verify|assume|require|refine|derive|allocate)\s)\brequirement\s+(?!def\b)(?:<\s*(${ID})\s*>\s*)?(${ID})(?:\s*:\s*(${QNAME}))?(?:\s*:>\s*(${QNAME}))?`, 'g')
  for (let m; (m = useRe.exec(text)); ) {
    usages.push({ name: unquote(m[2]), shortName: unquote(m[1]) ?? null, type: m[3] ? unquote(m[3]) : null, general: m[4] ? unquote(m[4]) : null, doc: docAfter(m.index + m[0].length), attributes: numericAttributes(topLevel(bodyAfter(text, m.index + m[0].length)).plain), attributions: costAttributions(topLevel(bodyAfter(text, m.index + m[0].length)).blocks), tags: complianceTags(topLevel(bodyAfter(text, m.index + m[0].length)).blocks) })
  }
  const relations = []
  const relRe = new RegExp(String.raw`\b(${KINDS.join('|')})\s+(?:requirement\s+)?(${QNAME})(?:\s+(?:by|from|to)\s+(${QNAME}))?\s*;`, 'g')
  for (let m; (m = relRe.exec(text)); ) relations.push({ kind: m[1], requirement: unquote(m[2]), target: m[3] ? unquote(m[3]) : null })
  return { definitions, usages, relations }
}

/** Declared requirements with their ids, text and numeric attributes: [{id, name, doc, attributes, kind: 'definition'|'usage'}]. */
export function declaredRequirements(source) {
  const s = scanRequirements(source)
  return [
    ...s.definitions.map((d) => ({ id: d.shortName || d.name, name: d.name, doc: d.doc, attributes: d.attributes, kind: 'definition', attributions: d.attributions, tags: d.tags })),
    ...s.usages.map((u) => ({ id: u.shortName || u.name, name: u.name, doc: u.doc, attributes: u.attributes, kind: 'usage', attributions: u.attributions, tags: u.tags })),
  ]
}

/** Requirement identifiers a source file declares or relates to: short name when present, else the declared name. */
export function requirementIds(source) {
  const s = scanRequirements(source)
  const ids = new Set()
  for (const d of s.definitions) ids.add(d.shortName || d.name)
  for (const u of s.usages) ids.add(u.shortName || u.name)
  return [...ids]
}

/** Trace links implied by a file: {requirement, relation, target, artifact}. */
export function tracesOf(path, source) {
  const s = scanRequirements(source)
  const alias = new Map()
  for (const u of s.usages) if (u.shortName) alias.set(u.name, u.shortName)
  for (const d of s.definitions) if (d.shortName) alias.set(d.name, d.shortName)
  const idOf = (name) => alias.get(name) || name
  const out = []
  for (const d of s.definitions) out.push({ requirement: d.shortName || d.name, relation: 'declares', target: null, artifact: path })
  for (const u of s.usages) {
    out.push({ requirement: u.shortName || u.name, relation: 'declares', target: null, artifact: path })
    if (u.general) out.push({ requirement: u.shortName || u.name, relation: 'derive', target: idOf(u.general), artifact: path })
  }
  for (const r of s.relations) out.push({ requirement: idOf(r.requirement), relation: r.kind, target: r.target, artifact: path })
  return out
}
