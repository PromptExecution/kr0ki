// Compliance tags on requirements and a coverage report against the owner's AI-governance chain:
//   EU AI Act -> Australian AI guardrails (VAISS 2024 / Guidance for AI Adoption 2025) -> ISO/IEC 42001 -> NIST AI RMF -> CIRMP / AESCSF
// Tag syntax: `framework:control-id` (lowercase framework slug, control id verbatim): `au-ai6:P5`, `iso42001:A.6.2.6`,
// `nist-ai-rmf:GOVERN-1.1`, `eu-ai-act:art-9`, `cirmp:supply-chain`, `aescsf:SP-2`.
// A tag finer than a catalogue control counts toward its parent (`iso42001:A.6.2.6` covers `A.6`; `GOVERN-1.1` covers `GOVERN-1` and `GOVERN`).
//
// SOURCE: docs/evaluations/AI-GOVERNANCE-crosswalk-2026-10-01.md. `confidence`: 'confirmed' = seen in a fetched source that session;
// 'recalled' = from the published text as remembered, NOT re-fetched. This is a working aid, not legal advice: verify against the
// official text (ISO 42001 is paywalled) before relying on an id. Dates change: the EU "Digital Omnibus" shifted high-risk dates.

export const FRAMEWORKS = {
  'eu-ai-act': {
    name: 'EU AI Act (Regulation (EU) 2024/1689)', url: 'https://eur-lex.europa.eu/eli/reg/2024/1689/oj', confidence: 'recalled',
    note: 'High-risk obligations deferred by the Digital Omnibus (Reg. (EU) 2026/1744, in force 27 Jul 2026, per a secondary source): Annex III 2 Dec 2027, Annex I 2 Aug 2028. Confirm on EUR-Lex.',
    controls: {
      'art-4': 'AI literacy', 'art-5': 'Prohibited practices', 'art-9': 'Risk management system', 'art-10': 'Data and data governance',
      'art-11': 'Technical documentation', 'art-12': 'Record-keeping', 'art-13': 'Transparency and information to deployers', 'art-14': 'Human oversight',
      'art-15': 'Accuracy, robustness and cybersecurity', 'art-16': 'Obligations of providers', 'art-17': 'Quality management system',
      'art-26': 'Obligations of deployers', 'art-27': 'Fundamental rights impact assessment', 'art-50': 'Transparency for certain AI systems',
      'art-72': 'Post-market monitoring', 'art-73': 'Serious incident reporting',
    },
  },
  'au-ai6': {
    name: 'Australian Guidance for AI Adoption: 6 essential practices (21 Oct 2025)', url: 'https://www.industry.gov.au/publications/guidance-for-ai-adoption', confidence: 'confirmed',
    note: 'Supersedes the VAISS 10 guardrails (see au-vaiss). The proposed mandatory guardrails were dropped (National AI Plan, 2 Dec 2025, per secondary sources).',
    controls: { P1: 'Decide who is accountable', P2: 'Understand impacts and plan accordingly', P3: 'Measure and manage risks', P4: 'Share information', P5: 'Test and monitor', P6: 'Maintain human control' },
  },
  'au-vaiss': {
    name: 'Australian Voluntary AI Safety Standard: 10 guardrails (Sept 2024; superseded by au-ai6)', url: 'https://www.industry.gov.au/publications/voluntary-ai-safety-standard', confidence: 'recalled',
    controls: {
      G1: 'Accountability, governance and strategy', G2: 'Risk management process', G3: 'Protect AI systems; data governance and quality', G4: 'Test, evaluate and monitor',
      G5: 'Human control or oversight', G6: 'Inform end-users about AI decisions and interactions', G7: 'Processes to challenge use or outcomes', G8: 'Transparency across the AI supply chain',
      G9: 'Keep records for third-party assessment', G10: 'Engage stakeholders (safety, diversity, fairness)',
    },
  },
  iso42001: {
    name: 'ISO/IEC 42001:2023 AI management system', url: 'https://www.iso.org/standard/81230.html', confidence: 'recalled',
    note: 'Group level only. Annex A sub-control titles beyond A.2.x/A.4.x ids are unverified; the standard is paywalled.',
    controls: {
      'cl.4': 'Context of the organization', 'cl.5': 'Leadership', 'cl.6': 'Planning (risk, AI risk and impact assessment, objectives)', 'cl.7': 'Support', 'cl.8': 'Operation',
      'cl.9': 'Performance evaluation', 'cl.10': 'Improvement', 'A.2': 'Policies related to AI', 'A.3': 'Internal organization', 'A.4': 'Resources for AI systems',
      'A.5': 'Assessing impacts of AI systems', 'A.6': 'AI system life cycle', 'A.7': 'Data for AI systems', 'A.8': 'Information for interested parties',
      'A.9': 'Use of AI systems', 'A.10': 'Third-party and customer relationships',
    },
  },
  'nist-ai-rmf': {
    name: 'NIST AI RMF 1.0 (NIST AI 100-1)', url: 'https://doi.org/10.6028/NIST.AI.100-1', confidence: 'recalled',
    note: 'Category counts per function are recalled; subcategory ids (GOVERN-1.1) count toward their category.',
    controls: {
      GOVERN: 'Govern (function)', MAP: 'Map (function)', MEASURE: 'Measure (function)', MANAGE: 'Manage (function)',
      ...Object.fromEntries([['GOVERN', 6], ['MAP', 5], ['MEASURE', 4], ['MANAGE', 4]].flatMap(([f, n]) => Array.from({ length: n }, (_, i) => [`${f}-${i + 1}`, `${f[0]}${f.slice(1).toLowerCase()} category ${i + 1}`]))),
    },
  },
  cirmp: {
    name: 'Critical Infrastructure Risk Management Program (SOCI Act 2018, Part 2A; CIRMP Rules)', url: 'https://www.cisc.gov.au/', confidence: 'confirmed',
    note: 'Enhanced CIRMP Rules registered 9 Jun 2026 (single secondary source): add FOCI risk, vetting, phishing-resistant MFA. Confirm on cisc.gov.au.',
    controls: { cyber: 'Cyber and information security hazard', personnel: 'Personnel hazard', 'supply-chain': 'Supply chain hazard', physical: 'Physical and natural hazard' },
  },
  aescsf: {
    name: 'AESCSF (Australian Energy Sector Cyber Security Framework)', url: 'https://aemo.com.au/', confidence: 'confirmed',
    note: 'Security Profiles SP-1..3 by criticality and Maturity Indicator Levels MIL-1..3; the exact 2025 domain list is unverified so domains are not catalogued.',
    controls: { 'SP-1': 'Security Profile 1', 'SP-2': 'Security Profile 2', 'SP-3': 'Security Profile 3', 'MIL-1': 'Maturity Indicator Level 1', 'MIL-2': 'Maturity Indicator Level 2', 'MIL-3': 'Maturity Indicator Level 3' },
  },
}

export const TAG_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*:[A-Za-z0-9]+(?:[-./][A-Za-z0-9]+)*$/

/** @returns {{framework:string, control:string}|null} null when the text is not a well-formed tag */
export function parseTag(tag) {
  if (typeof tag !== 'string' || !TAG_RE.test(tag)) return null
  const i = tag.indexOf(':')
  return { framework: tag.slice(0, i), control: tag.slice(i + 1) }
}

/** The catalogue control a (possibly finer) control id counts toward, or null. Tries the id, then trims `.x` and `-x` tails. */
export function resolveControl(framework, control) {
  const catalogue = FRAMEWORKS[framework]?.controls
  if (!catalogue) return null
  let id = control
  while (id) {
    if (Object.hasOwn(catalogue, id)) return id
    const next = id.replace(/[.\-/][^.\-/]*$/, '')
    if (next === id) return null
    id = next
  }
  return null
}

/** The catalogue controls a tag covers: the control itself and its parents (GOVERN-1.1 -> GOVERN-1 and GOVERN). */
function covered(framework, control) {
  const hit = resolveControl(framework, control)
  if (!hit) return []
  const out = [hit]
  const catalogue = FRAMEWORKS[framework].controls
  let id = hit
  for (;;) {
    const parent = id.replace(/[.\-/][^.\-/]*$/, '')
    if (parent === id) break
    if (Object.hasOwn(catalogue, parent)) out.push(parent)
    id = parent
  }
  return out
}

/**
 * Coverage of a framework by the requirements in a graph.
 * @returns {{ framework, name, rows: {control,name,requirements:string[],judgement:number,official:number}[], gaps:string[],
 *             malformed:{requirement,tag}[], unknown:{requirement,tag}[], untagged:string[] }}
 */
export function complianceCoverage(graph, framework, { include = null } = {}) {
  const fw = FRAMEWORKS[framework]
  if (!fw) throw new Error(`unknown framework ${framework}`)
  const inScenario = (id) => include === null || include.has(id)
  const rows = Object.entries(fw.controls).map(([control, name]) => ({ control, name, requirements: new Set(), judgement: 0, official: 0 }))
  const byControl = new Map(rows.map((r) => [r.control, r]))
  const malformed = [], unknown = [], untagged = []
  for (const n of graph.nodes.filter((x) => inScenario(x.id))) {
    if (!n.tags.length) untagged.push(n.id)
    for (const t of n.tags) {
      const p = parseTag(t.tag)
      if (!p) { malformed.push({ requirement: n.id, tag: t.tag }); continue }
      if (p.framework !== framework) continue
      const hits = covered(framework, p.control)
      if (!hits.length) { unknown.push({ requirement: n.id, tag: t.tag }); continue }
      for (const h of hits) {
        const row = byControl.get(h)
        if (!row.requirements.has(n.id)) { row.requirements.add(n.id); if (t.basis === 'official') row.official++; else row.judgement++ }
      }
    }
  }
  const out = rows.map((r) => ({ ...r, requirements: [...r.requirements].sort() }))
  return { framework, name: fw.name, rows: out, gaps: out.filter((r) => !r.requirements.length).map((r) => r.control), malformed, unknown, untagged }
}
