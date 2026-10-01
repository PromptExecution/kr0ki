// Turn the server's normalized ReqIF import (POST /requirements/import) into project files: the original .reqif is kept as
// an artifact, and a SysML v2 text file declares each requirement so the project is SysML-native and traceable.
// `derives` relations become SysML specialization (`:>`); other relation kinds are kept as traces (they need parts to satisfy).

// ReqIF/ufo relation kinds -> the verbs used in SysML v2 and in project traces
const KIND = { derives: 'derive', refines: 'refine', satisfies: 'satisfy', verifies: 'verify', traces: 'trace', implements: 'implement', allocatedto: 'allocate', contains: 'contain', requires: 'require' }
const SAFE = (id) => String(id).replace(/[^A-Za-z0-9_]/g, '_').replace(/^(\d)/, '_$1')
const esc = (s) => String(s ?? '').replace(/\*\//g, '* /') // a doc comment must not close itself

/** @returns {{ sysml: string, traces: {requirement:string, relation:string, target:string|null, artifact:string}[], count: number }} */
export function reqifToSysml(imported, { sysmlPath, reqifPath, packageName = 'ImportedRequirements' } = {}) {
  const docs = imported?.documents || []
  const reqs = docs.flatMap((d) => d.graph?.requirements || [])
  const rels = docs.flatMap((d) => d.graph?.relations || [])
  const names = new Map(reqs.map((r) => [r.id, `req_${SAFE(r.id)}`]))
  const derivedFrom = new Map()
  const traces = []
  for (const r of rels) {
    const kind = KIND[String(r.kind || 'trace').toLowerCase()] || String(r.kind || 'trace').toLowerCase()
    if (kind === 'derive' && names.has(r.target) && names.has(r.source)) { derivedFrom.set(r.source, r.target); continue }
    traces.push({ requirement: r.source, relation: kind, target: r.target, artifact: reqifPath })
  }
  const lines = [`package ${SAFE(packageName)} {`]
  for (const r of reqs) {
    const general = derivedFrom.has(r.id) ? ` :> ${names.get(derivedFrom.get(r.id))}` : ''
    const doc = esc(r.text || r.title || '')
    const cost = r.attributes && /^-?\d+(\.\d+)?$/.test(String(r.attributes.cost ?? '')) ? `\n    attribute cost = ${r.attributes.cost};` : ''
    lines.push(`  requirement <'${String(r.id).replace(/'/g, '')}'> ${names.get(r.id)}${general} {`)
    lines.push(`    doc /* ${doc} */${cost}`)
    lines.push('  }')
    traces.push({ requirement: r.id, relation: 'declares', target: null, artifact: reqifPath })
  }
  lines.push('}')
  return { sysml: lines.join('\n') + '\n', traces: traces.filter((t) => t.relation !== 'declares'), count: reqs.length }
}
