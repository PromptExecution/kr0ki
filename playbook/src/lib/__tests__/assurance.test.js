import { describe, expect, it, vi } from 'vitest'
import {
  AssuranceError, STATES, createAssuranceClient, describeGap, driftText, explainError, gapKind, shortRev, sourceKindInfo, stateInfo,
} from '../assurance.js'

const ok = (body) => ({ ok: true, status: 200, json: async () => body, text: async () => String(body), blob: async () => body })
const fail = (status, body, statusText = '') => ({ ok: false, status, statusText, json: async () => { if (body === undefined) throw new Error('no json'); return body } })

describe('states', () => {
  it('has five distinct states, each with a label and a symbol (never colour alone)', () => {
    expect(STATES.map((s) => s.id)).toEqual(['unsatisfied', 'satisfied_untested', 'verified', 'failing', 'stale'])
    expect(new Set(STATES.map((s) => s.symbol)).size).toBe(5)
    expect(new Set(STATES.map((s) => s.label)).size).toBe(5)
    for (const s of STATES) expect(s.hint.length).toBeGreaterThan(10)
  })
  it('keeps "satisfied · untested" and "verified" apart', () => {
    expect(stateInfo('satisfied_untested').label).not.toBe(stateInfo('verified').label)
  })
  it('does not throw on an unknown state', () => {
    expect(stateInfo('mystery').label).toBe('mystery')
  })
})

describe('source kinds', () => {
  it('keeps guidance, policy and binding obligations distinguishable', () => {
    const labels = ['binding_obligation', 'organisational_policy', 'guidance'].map((k) => sourceKindInfo(k).label)
    expect(new Set(labels).size).toBe(3)
    expect(sourceKindInfo(undefined).label).toBe('unknown')
  })
})

describe('gaps', () => {
  it('names every gap kind the server can report, with the specifics where it has them', () => {
    expect(gapKind({ gap: 'stale' })).toBe('stale')
    expect(describeGap({ gap: 'control_not_implemented', control_id: 'CTL-A06' })).toContain('CTL-A06')
    expect(describeGap({ gap: 'dangling_element', element_id: 'X::Y' })).toContain('X::Y')
    expect(describeGap({ gap: 'unknown_verification_case', verification_id: 'VC-9' })).toContain('VC-9')
    for (const g of ['no_source', 'unsatisfied', 'no_enforcing_control', 'no_verification_case', 'satisfied_untested', 'failing', 'stale']) {
      expect(describeGap({ gap: g }).length).toBeGreaterThan(10)
    }
    expect(describeGap({ gap: 'brand_new' })).toContain('brand_new')
    expect(gapKind(null)).toBe('unknown')
  })
})

describe('revisions', () => {
  it('shortens digests and git revisions but keeps a dirty marker visible', () => {
    expect(shortRev('sha256:0213bc9c3286628f4e7b25cac229104a3f82185fbb7ef54a0735222ca3a67d4b')).toBe('0213bc9c3286')
    expect(shortRev('abb46f1005146b6c17157dd7d6af2ec795c04fc3')).toBe('abb46f1005')
    expect(shortRev('abb46f1005146b6c17157dd7d6af2ec795c04fc3+dirty.7b2d45a0e4f4')).toBe('abb46f1005+dirty.7b2d4')
    expect(shortRev('')).toBe('—')
    expect(shortRev(null)).toBe('—')
    expect(shortRev('impl-1')).toBe('impl-1')
  })
  it('says which of the three revisions a stale result no longer matches', () => {
    expect(driftText({ state: 'fresh' })).toBe('')
    expect(driftText({ state: 'stale', drift: { model: false, implementation: true, configuration: false } })).toBe('implementation changed')
    expect(driftText({ state: 'stale', drift: { model: true, implementation: false, configuration: true } })).toBe('model + configuration changed')
    expect(driftText({ state: 'stale' })).toBe('revisions changed')
    expect(driftText(undefined)).toBe('')
  })
})

describe('errors are explained without leaking a token', () => {
  const e = (status, body) => new AssuranceError(status, body)
  it('maps statuses to what the person should do', () => {
    expect(explainError(e(401, { error: 'unauthorized' }))).toMatch(/access token/)
    expect(explainError(e(403, { error: 'forbidden', message: 'identity `reader` has no `model.commit` grant' }))).toMatch(/not allowed.*model\.commit/)
    expect(explainError(e(503, { error: 'assurance_not_configured', message: 'x' }))).toMatch(/not enabled/)
    expect(explainError(e(503, { error: 'audit_unavailable', message: 'x' }))).toMatch(/audit log/)
    expect(explainError(e(409, { error: 'revision_mismatch', message: 'asked X, at Y' }))).toMatch(/moved on/)
    expect(explainError(e(409, { error: 'stale_base', message: 'm' }))).toMatch(/Stale base/)
    expect(explainError(e(500, { message: 'boom' }))).toBe('boom')
    expect(explainError(new TypeError('Failed to fetch'))).toMatch(/Could not reach/)
  })
  it('falls back to the status text, then the status', () => {
    expect(new AssuranceError(502, null, 'Bad Gateway').message).toBe('Bad Gateway')
    expect(new AssuranceError(502, null).message).toBe('HTTP 502')
  })
})

describe('client', () => {
  const client = (fetchImpl, token = 'tok-secret') => createAssuranceClient({ baseUrl: 'http://k.test:8787/', token, fetchImpl, requestId: () => 'rid-1' })

  it('sends the bearer token in the header only, plus a correlation id', async () => {
    const f = vi.fn().mockResolvedValue(ok({ total: 0, requirements: [] }))
    await client(f).list({ state: 'verified', owner: '', gap: undefined })
    const [url, init] = f.mock.calls[0]
    expect(url).toBe('http://k.test:8787/assurance/requirements?state=verified')
    expect(init.headers.Authorization).toBe('Bearer tok-secret')
    expect(init.headers['X-Request-ID']).toBe('rid-1')
    expect(url).not.toContain('tok-secret')
  })
  it('sends no Authorization header without a token', async () => {
    const f = vi.fn().mockResolvedValue(ok({}))
    await client(f, '').list()
    expect(f.mock.calls[0][1].headers.Authorization).toBeUndefined()
  })
  it('encodes ids so they cannot change the route', async () => {
    const f = vi.fn().mockResolvedValue(ok({}))
    await client(f).requirement('../admin?x=1')
    expect(f.mock.calls[0][0]).toBe('http://k.test:8787/assurance/requirements/..%2Fadmin%3Fx%3D1')
    await client(f).trace('KR-A01')
    expect(f.mock.calls[1][0]).toBe('http://k.test:8787/assurance/requirements/KR-A01/trace')
  })
  it('targets the right routes and methods', async () => {
    const f = vi.fn().mockResolvedValue(ok({}))
    const c = client(f)
    await c.evidence({ requirement: 'KR-A01', case: '' })
    await c.view()
    await c.viewSvg()
    await c.viewTable()
    await c.verify('VC-A01', 'abb46f1')
    await c.audit({ decision: 'deny' })
    expect(f.mock.calls.map(([u, i]) => `${i.method} ${u.replace('http://k.test:8787', '')}`)).toEqual([
      'GET /assurance/evidence?requirement=KR-A01',
      'GET /assurance/view?format=json',
      'GET /assurance/view?format=svg',
      'GET /assurance/view?format=table',
      'POST /assurance/verify/VC-A01?revision=abb46f1',
      'GET /assurance/audit?decision=deny',
    ])
  })
  it('turns a server refusal into an AssuranceError carrying status, code and message', async () => {
    const f = vi.fn().mockResolvedValue(fail(403, { error: 'forbidden', message: 'no grant', request_id: 'r' }))
    await expect(client(f).list()).rejects.toMatchObject({ name: 'AssuranceError', status: 403, code: 'forbidden', message: 'no grant' })
  })
  it('survives a non-JSON error body', async () => {
    const f = vi.fn().mockResolvedValue(fail(502, undefined, 'Bad Gateway'))
    await expect(client(f).list()).rejects.toMatchObject({ status: 502, message: 'Bad Gateway' })
  })
  it('never puts the token in an error', async () => {
    const f = vi.fn().mockResolvedValue(fail(401, { error: 'unauthorized', message: 'missing or invalid bearer token' }))
    const err = await client(f).list().catch((x) => x)
    expect(JSON.stringify([err.message, err.body])).not.toContain('tok-secret')
  })
})
