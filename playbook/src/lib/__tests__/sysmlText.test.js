import { describe, expect, it } from 'vitest'
import { CODE_RE, declaredRequirements, requirementIds, scanRequirements, tracesOf } from '../sysmlText.js'

const model = `
package Vehicle {
  // requirement def Commented { }
  requirement def MassReq {
    doc /* The total mass
           * shall not exceed 1000 kg. */
    attribute massActual : Real;
    require constraint { massActual <= 1000 }
  }
  requirement def <'SYS-9'> RangeReq { doc /* Range >= 400 km */ }
  requirement <'REQ-1'> vehicleMass : MassReq;
  requirement vehicleRange : RangeReq;
  requirement <'REQ-3'> derivedMass :> vehicleMass { doc /* Derived from REQ-1 */ }
  part vehicle { part engine; }
  satisfy vehicleMass by vehicle;
  satisfy requirement vehicleRange by vehicle.engine;
  verification def MassTest { verify vehicleMass; }
  allocate vehicleRange to vehicle;
}`

describe('scanRequirements', () => {
  const s = scanRequirements(model)

  it('finds definitions and usages with ids, types, generals and doc text', () => {
    expect(s.definitions.map((d) => d.name)).toEqual(['MassReq', 'RangeReq'])
    expect(s.definitions[0].doc).toBe('The total mass shall not exceed 1000 kg.')
    expect(s.definitions[1]).toMatchObject({ shortName: 'SYS-9', doc: 'Range >= 400 km' })
    expect(s.usages.map((u) => [u.shortName, u.name, u.type, u.general])).toEqual([
      ['REQ-1', 'vehicleMass', 'MassReq', null],
      [null, 'vehicleRange', 'RangeReq', null],
      ['REQ-3', 'derivedMass', null, 'vehicleMass'],
    ])
    expect(s.usages[2].doc).toBe('Derived from REQ-1')
  })

  it('does not mistake commented-out code, "requirement def", or relationship keywords for usages', () => {
    expect(s.definitions.some((d) => d.name === 'Commented')).toBe(false)
    expect(s.usages.map((u) => u.name)).not.toContain('def')
    expect(s.usages.map((u) => u.name)).not.toContain('vehicleRange\n') // `satisfy requirement vehicleRange` is a relation, not a declaration
    expect(s.usages.filter((u) => u.name === 'vehicleRange')).toHaveLength(1)
  })

  it('finds satisfy / verify / allocate relations, with and without the word "requirement"', () => {
    expect(s.relations).toEqual([
      { kind: 'satisfy', requirement: 'vehicleMass', target: 'vehicle' },
      { kind: 'satisfy', requirement: 'vehicleRange', target: 'vehicle.engine' },
      { kind: 'verify', requirement: 'vehicleMass', target: null },
      { kind: 'allocate', requirement: 'vehicleRange', target: 'vehicle' },
    ])
  })

  it('handles quoted names, empty input and garbage without throwing', () => {
    expect(scanRequirements("requirement <'R 1'> 'my req' : T;").usages[0]).toMatchObject({ shortName: 'R 1', name: 'my req', type: 'T' })
    expect(scanRequirements('')).toEqual({ definitions: [], usages: [], relations: [] })
    expect(scanRequirements(null).usages).toEqual([])
    expect(() => scanRequirements('requirement {{{ <<< satisfy by ; doc /* unterminated')).not.toThrow()
  })
})

describe('requirementIds and tracesOf', () => {
  it('uses the short name as the identifier when there is one, else the declared name', () => {
    expect(requirementIds(model).sort()).toEqual(['REQ-1', 'REQ-3', 'SYS-9', 'vehicleRange'].sort().concat(['MassReq']).sort())
  })

  it('reports declarations and links per artifact, resolving names to their identifiers', () => {
    const t = tracesOf('model/vehicle.sysml', model)
    expect(t).toContainEqual({ requirement: 'REQ-1', relation: 'declares', target: null, artifact: 'model/vehicle.sysml' })
    expect(t).toContainEqual({ requirement: 'REQ-1', relation: 'satisfy', target: 'vehicle', artifact: 'model/vehicle.sysml' }) // vehicleMass -> REQ-1
    expect(t).toContainEqual({ requirement: 'REQ-1', relation: 'verify', target: null, artifact: 'model/vehicle.sysml' })
    expect(t).toContainEqual({ requirement: 'REQ-3', relation: 'derive', target: 'REQ-1', artifact: 'model/vehicle.sysml' }) // :> vehicleMass
    expect(t).toContainEqual({ requirement: 'vehicleRange', relation: 'allocate', target: 'vehicle', artifact: 'model/vehicle.sysml' })
  })
})

describe('numeric attributes (the cost model)', () => {
  const src = `package P {
    requirement <'R1'> a { doc /* A */ attribute cost = 120.5; attribute mass : Real = 3 [kg]; attribute note = "text"; attribute calc = cost * 2; }
    requirement <'R2'> b;
    requirement <'R3'> c { requirement <'R3.1'> inner { attribute cost = 7; } attribute cost = 30; }
    requirement def D { attribute cost = -4; attribute e = 1.5e2; }
  }`
  it('reads numeric literals (with optional type and unit) and ignores strings and expressions', () => {
    const r = Object.fromEntries(declaredRequirements(src).map((x) => [x.id, x]))
    expect(r.R1.attributes).toEqual({ cost: 120.5, mass: 3 })
    expect(r.R2.attributes).toEqual({}) // no body at all
    expect(r.D.attributes).toEqual({ cost: -4, e: 150 })
  })

  it('a nested requirement keeps its own attributes and never leaks into (or out of) its parent, in either source order', () => {
    const r = Object.fromEntries(declaredRequirements(src).map((x) => [x.id, x]))
    expect(r['R3.1'].attributes).toEqual({ cost: 7 })
    expect(r.R3.attributes).toEqual({ cost: 30 })
    const reversed = Object.fromEntries(declaredRequirements("requirement <'P'> p { attribute cost = 30; requirement <'C'> c { attribute cost = 7; } }").map((x) => [x.id, x]))
    expect(reversed.P.attributes).toEqual({ cost: 30 })
    const noOwn = Object.fromEntries(declaredRequirements("requirement <'P'> p { requirement <'C'> c { attribute cost = 7; } }").map((x) => [x.id, x]))
    expect(noOwn.P.attributes).toEqual({}) // the inner cost must not be attributed to the parent
  })
})

describe('cost attribution metadata (accounting-style codes, not amounts)', () => {
  const src = `package P {
    requirement <'R1'> a {
      doc /* A */
      @CostAttribution { code = 'CC-4410/WBS-2.3'; share = 0.6; }
      @CostAttribution { code = "CC-9000/ACT-1"; share = 0.4; }
    }
    requirement <'R2'> b { metadata CostAttribution { code = 'CC-4410/WBS-2.4'; } }
    requirement <'R3'> outer {
      @CostAttribution { code = 'CC-1'; share = 0.5; }
      requirement <'R3.1'> inner { @CostAttribution { code = 'CC-2'; share = 1; } }
    }
    requirement <'R4'> none { doc /* no attribution */ }
    requirement <'R5'> noCode { @CostAttribution { share = 0.5; } }
    requirement <'R6'> other { @SomethingElse { code = 'CC-X'; } }
  }`
  const r = Object.fromEntries(declaredRequirements(src).map((x) => [x.id, x]))

  it('reads one or more attribution lines per requirement, in both metadata spellings, with share defaulting to 1', () => {
    expect(r.R1.attributions).toEqual([{ code: 'CC-4410/WBS-2.3', share: 0.6 }, { code: 'CC-9000/ACT-1', share: 0.4 }])
    expect(r.R2.attributions).toEqual([{ code: 'CC-4410/WBS-2.4', share: 1 }])
  })

  it('keeps nested requirements\' attributions to themselves, in either direction', () => {
    expect(r.R3.attributions).toEqual([{ code: 'CC-1', share: 0.5 }])
    expect(r['R3.1'].attributions).toEqual([{ code: 'CC-2', share: 1 }])
  })

  it('ignores metadata that is not CostAttribution or has no code, and a requirement with none has none', () => {
    expect(r.R4.attributions).toEqual([])
    expect(r.R5.attributions).toEqual([])
    expect(r.R6.attributions).toEqual([])
  })

  it('never turns an attribution into a number: the legacy numeric attribute is a separate, optional field', () => {
    expect(r.R1.attributes).toEqual({})
  })

  it('CODE_RE accepts hierarchical accounting codes and rejects anything else', () => {
    for (const ok of ['CC-4410', 'CC-4410/WBS-2.3', 'CC-4410/WBS-2.3/ACT-07', 'a/b/c', 'X_1.2-3']) expect(CODE_RE.test(ok), ok).toBe(true)
    for (const bad of ['', '/CC', 'CC/', 'CC//X', 'CC 1', "CC'; DROP", 'CC/..', '$5']) expect(CODE_RE.test(bad), bad).toBe(false)
  })
})
