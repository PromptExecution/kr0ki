import { describe, expect, it } from 'vitest'
import { FRAMEWORKS, complianceCoverage, parseTag, resolveControl } from '../compliance.js'
import { buildRequirementGraph } from '../requirementsGraph.js'

const R = (id, tags) => ({ id, doc: id, tags: tags.map((t) => (typeof t === 'string' ? { tag: t, basis: 'judgement' } : t)) })
const graph = (reqs) => buildRequirementGraph([], reqs)

describe('tags', () => {
  it('parses framework:control and rejects anything else', () => {
    expect(parseTag('au-ai6:P5')).toEqual({ framework: 'au-ai6', control: 'P5' })
    expect(parseTag('iso42001:A.6.2.6')).toEqual({ framework: 'iso42001', control: 'A.6.2.6' })
    expect(parseTag('nist-ai-rmf:GOVERN-1.1')).toEqual({ framework: 'nist-ai-rmf', control: 'GOVERN-1.1' })
    for (const bad of ['', 'P5', ':P5', 'au-ai6:', 'AU-AI6:P5', 'a b:c', 'x:y;z', "x:'; drop", 'au-ai6:P5 ', null, 7]) expect(parseTag(bad), String(bad)).toBeNull()
  })

  it('a finer id resolves to its catalogue control, and unknown ones resolve to nothing', () => {
    expect(resolveControl('iso42001', 'A.6.2.6')).toBe('A.6')
    expect(resolveControl('nist-ai-rmf', 'GOVERN-1.1')).toBe('GOVERN-1')
    expect(resolveControl('eu-ai-act', 'art-9')).toBe('art-9')
    expect(resolveControl('au-ai6', 'P9')).toBeNull()
    expect(resolveControl('nope', 'x')).toBeNull()
    expect(resolveControl('iso42001', '__proto__')).toBeNull() // catalogue lookups are own-property only
  })

  it('every catalogued control id is itself a well-formed tag, and every framework carries provenance', () => {
    for (const [slug, fw] of Object.entries(FRAMEWORKS)) {
      expect(['confirmed', 'recalled']).toContain(fw.confidence)
      expect(fw.url).toMatch(/^https:\/\//)
      for (const control of Object.keys(fw.controls)) expect(parseTag(`${slug}:${control}`), `${slug}:${control}`).not.toBeNull()
    }
  })
})

describe('complianceCoverage', () => {
  const g = graph([
    R('R1', ['au-ai6:P5', 'iso42001:A.6.2.6', 'nist-ai-rmf:GOVERN-1.1']),
    R('R2', ['au-ai6:P5', { tag: 'au-ai6:P3', basis: 'official' }, 'au-ai6:P9', 'bad tag']),
    R('R3', []),
    R('R4', ['eu-ai-act:art-9']),
  ])

  it('lists, per control, which requirements cover it and how many rest on judgement vs official mappings', () => {
    const c = complianceCoverage(g, 'au-ai6')
    const row = Object.fromEntries(c.rows.map((r) => [r.control, r]))
    expect(row.P5).toMatchObject({ requirements: ['R1', 'R2'], judgement: 2, official: 0 })
    expect(row.P3).toMatchObject({ requirements: ['R2'], official: 1 })
    expect(c.gaps).toEqual(['P1', 'P2', 'P4', 'P6'])
  })

  it('reports malformed and unknown tags and untagged requirements instead of ignoring them', () => {
    const c = complianceCoverage(g, 'au-ai6')
    expect(c.malformed).toEqual([{ requirement: 'R2', tag: 'bad tag' }])
    expect(c.unknown).toEqual([{ requirement: 'R2', tag: 'au-ai6:P9' }])
    expect(c.untagged).toEqual(['R3'])
  })

  it('counts a finer tag toward its parents and only against its own framework', () => {
    const iso = Object.fromEntries(complianceCoverage(g, 'iso42001').rows.map((r) => [r.control, r]))
    expect(iso['A.6'].requirements).toEqual(['R1'])
    const nist = Object.fromEntries(complianceCoverage(g, 'nist-ai-rmf').rows.map((r) => [r.control, r]))
    expect(nist['GOVERN-1'].requirements).toEqual(['R1'])
    expect(nist.GOVERN.requirements).toEqual(['R1']) // the function covers its categories' tags
    expect(complianceCoverage(g, 'eu-ai-act').rows.find((r) => r.control === 'art-9').requirements).toEqual(['R4'])
  })

  it('honours the scenario and refuses an unknown framework', () => {
    expect(complianceCoverage(g, 'au-ai6', { include: new Set(['R1']) }).rows.find((r) => r.control === 'P5').requirements).toEqual(['R1'])
    expect(() => complianceCoverage(g, 'iso9001')).toThrow(/unknown framework/)
  })
})
