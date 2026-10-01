import { describe, expect, it } from 'vitest'
import { buildRequirementGraph, coverageGaps, duplicateIds, rollUpCosts, toD2 } from '../requirementsGraph.js'

const T = (requirement, relation, target = null, artifact = 'a.sysml') => ({ requirement, relation, target, artifact })
const traces = [
  T('R1', 'declares'), T('R2', 'declares'), T('R3', 'declares'), T('R4', 'declares'),
  T('R2', 'derive', 'R1'), T('R3', 'derive', 'R2'), T('R4', 'derive', 'R1'),
  T('R1', 'satisfy', 'vehicle'), T('R1', 'verify'), T('R2', 'satisfy', 'engine'),
]
const declared = [
  { id: 'R1', doc: 'Top', attributes: { cost: 10 } }, { id: 'R2', doc: 'Mid', attributes: { cost: 20 } },
  { id: 'R3', doc: 'Leaf', attributes: { cost: 30 } }, { id: 'R4', doc: 'Other leaf' },
]
const g = buildRequirementGraph(traces, declared)

describe('buildRequirementGraph', () => {
  it('has one node per requirement with its cost, and derive edges from child to parent', () => {
    expect(g.nodes.map((n) => [n.id, n.cost])).toEqual([['R1', 10], ['R2', 20], ['R3', 30], ['R4', null]])
    expect(g.edges).toContainEqual({ from: 'R2', to: 'R1', kind: 'derive' })
    expect(g.edges).toHaveLength(3)
    expect(g.nodes[0].satisfiedBy).toEqual(['vehicle'])
  })

  it('normalises ReqIF-style relation names and does not duplicate edges', () => {
    const x = buildRequirementGraph([T('A', 'derives', 'B'), T('A', 'derive', 'B'), T('B', 'declares')], [])
    expect(x.edges).toEqual([{ from: 'A', to: 'B', kind: 'derive' }])
  })
})

describe('coverageGaps', () => {
  it('lists requirements that are unsatisfied and/or unverified', () => {
    const gaps = Object.fromEntries(coverageGaps(g).map((x) => [x.id, x]))
    expect(gaps.R1).toBeUndefined()
    expect(gaps.R2).toMatchObject({ unsatisfied: false, unverified: true })
    expect(gaps.R3).toMatchObject({ unsatisfied: true, unverified: true })
  })
})

describe('rollUpCosts', () => {
  it('rolls cost up through derived requirements; unpriced ones count 0 and are reported', () => {
    const r = rollUpCosts(g)
    expect(r.perRequirement.R1).toMatchObject({ own: 10, rollup: 60 }) // 10 + R2(20) + R3(30) + R4(0)
    expect(r.perRequirement.R2.rollup).toBe(50)
    expect(r.perRequirement.R3.rollup).toBe(30)
    expect(r.total).toBe(60)
    expect(r.unpriced).toEqual(['R4'])
  })

  it('a scenario excludes requirements and everything is recomputed', () => {
    const noMid = rollUpCosts(g, new Set(['R1', 'R3', 'R4']))
    expect(noMid.total).toBe(40)
    expect(noMid.perRequirement.R2.included).toBe(false)
    expect(noMid.perRequirement.R1.rollup).toBe(40) // R3 still derives (transitively) from R1 through the excluded R2
    expect(rollUpCosts(g, new Set()).total).toBe(0)
  })

  it('is cycle-safe and counts a shared descendant once', () => {
    const cyc = buildRequirementGraph([T('A', 'derive', 'B'), T('B', 'derive', 'A'), T('C', 'derive', 'A'), T('C', 'derive', 'B')], [
      { id: 'A', attributes: { cost: 1 } }, { id: 'B', attributes: { cost: 2 } }, { id: 'C', attributes: { cost: 4 } },
    ])
    const r = rollUpCosts(cyc)
    expect(r.perRequirement.A.rollup).toBe(7) // A + B + C, each once, despite A<->B and C under both
    expect(r.total).toBe(7)
  })
})

describe('toD2', () => {
  it('draws ids, titles, cost roll-ups, derive edges and marks gaps; quoted safely', () => {
    const d2 = toD2(g, rollUpCosts(g))
    expect(d2).toContain('"R1": "R1\\nTop\\ncost 10 (Σ 60)"')
    expect(d2).toContain('"R2" -> "R1": "derive"')
    expect(d2).toContain('"R3": "R3\\nLeaf\\ncost 30" { style.stroke: "#f87171" }') // a coverage gap
    const money = toD2(buildRequirementGraph([T('M', 'declares')], [{ id: 'M', doc: 'costs $5 and ${x}' }]))
    expect(money).toContain('costs \\$5 and \\${x}') // a literal $ must be escaped or D2 treats it as a substitution
    const evil = toD2(buildRequirementGraph([T('X"Y', 'declares')], [{ id: 'X"Y', doc: 'a "quoted"\nline' }]))
    expect(evil).toContain('"X\\"Y"')
    expect(evil).not.toMatch(/\n[^"\n]*"quoted"/)
  })
})

describe('duplicateIds', () => {
  it('reports an id declared in two different files, but not the same file twice', () => {
    const dup = buildRequirementGraph([T('R1', 'declares', null, 'a.sysml'), T('R1', 'declares', null, 'b.sysml'), T('R2', 'declares', null, 'a.sysml'), T('R2', 'declares', null, 'a.sysml')], [])
    expect(duplicateIds(dup)).toEqual([{ id: 'R1', files: ['a.sysml', 'b.sysml'] }])
    expect(duplicateIds(g)).toEqual([])
  })
})
