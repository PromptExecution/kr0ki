import { describe, expect, it } from 'vitest'
import { reqifToSysml } from '../reqif.js'
import { scanRequirements, tracesOf } from '../sysmlText.js'

const imported = { documents: [{ graph: {
  requirements: [
    { id: 'REQ-1', title: 'Encryption at rest', text: 'System shall encrypt data at rest.' },
    { id: 'REQ-2', title: 'Key rotation', text: 'Keys */ shall rotate yearly.', attributes: { cost: '12.5' } },
    { id: '3 odd id', title: 't', text: 'x' },
  ],
  relations: [{ id: 'REL-1', source: 'REQ-2', target: 'REQ-1', kind: 'derives' }, { id: 'REL-2', source: 'REQ-1', target: '3 odd id', kind: 'verifies' }],
} }] }

describe('reqifToSysml', () => {
  const out = reqifToSysml(imported, { sysmlPath: 'requirements/x.sysml', reqifPath: 'requirements/x.reqif' })

  it('declares every requirement with its ReqIF id as the SysML short name, and keeps the cost attribute', () => {
    const s = scanRequirements(out.sysml)
    expect(s.usages.map((u) => u.shortName)).toEqual(['REQ-1', 'REQ-2', '3 odd id'])
    expect(s.usages[1].attributes).toEqual({ cost: 12.5 })
    expect(out.count).toBe(3)
  })

  it('turns derives into specialization, which the scanner reads back as a derive trace', () => {
    expect(out.sysml).toContain(':> req_REQ_1')
    const t = tracesOf('requirements/x.sysml', out.sysml)
    expect(t).toContainEqual({ requirement: 'REQ-2', relation: 'derive', target: 'REQ-1', artifact: 'requirements/x.sysml' })
  })

  it('keeps other relation kinds as traces against the .reqif artifact, and cannot be broken by hostile text', () => {
    expect(out.traces).toEqual([{ requirement: 'REQ-1', relation: 'verify', target: '3 odd id', artifact: 'requirements/x.reqif' }])
    expect(out.sysml).not.toContain('*/ shall') // a doc comment cannot close itself
    expect(reqifToSysml({}, {}).sysml).toContain('package ImportedRequirements {')
  })
})
