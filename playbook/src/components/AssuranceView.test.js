import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import AssuranceView from './AssuranceView.vue'
import { AssuranceError, shortRev } from '../lib/assurance.js'
import list from './__fixtures__/assurance/list.json'
import detailA06 from './__fixtures__/assurance/requirement-KR-A06.json'
import traceA06 from './__fixtures__/assurance/trace-KR-A06.json'
import evidenceA06 from './__fixtures__/assurance/evidence-KR-A06.json'
import audit from './__fixtures__/assurance/audit.json'

const clone = (x) => structuredClone(x)
const refusal = (status, error, message) => new AssuranceError(status, { error, message })

/** A fake API client whose answers are the real fixtures; override any method. */
function fakeClient(over = {}) {
  return {
    list: vi.fn(async (f = {}) => {
      const l = clone(list)
      const rows = l.requirements.filter((r) => (!f.state || r.assurance === f.state) && (!f.owner || r.owner === f.owner) && (!f.status || r.status === f.status) && (!f.gap || r.gaps.some((g) => g.gap === f.gap)))
      return { ...l, total: rows.length, requirements: rows }
    }),
    requirement: vi.fn(async () => clone(detailA06)),
    trace: vi.fn(async () => clone(traceA06)),
    evidence: vi.fn(async () => clone(evidenceA06)),
    viewSvg: vi.fn(async () => new Blob(['<svg/>'], { type: 'image/svg+xml' })),
    audit: vi.fn(async () => clone(audit)),
    verify: vi.fn(async (caseId) => ({ revisions: {}, case_id: caseId, result: 'pass', detail: '9 tests passed', evidence_keys: [] })),
    ...over,
  }
}
async function mountView(client = fakeClient(), props = {}) {
  const w = mount(AssuranceView, { props: { client, rendererUrl: 'http://k.test:8787', ...props }, attachTo: document.body })
  await flushPromises()
  return w
}
const t = (w, id) => w.find(`[data-testid="${id}"]`)

beforeEach(() => {
  globalThis.URL.createObjectURL = vi.fn(() => 'blob:fake-diagram')
  globalThis.URL.revokeObjectURL = vi.fn()
})
afterEach(() => document.body.replaceChildren())

describe('the requirements table', () => {
  it('shows the revisions the answer was computed at, shortened', async () => {
    const w = await mountView()
    const revs = t(w, 'revisions').text()
    expect(revs).toContain('0213bc9c3286')
    expect(revs).toContain(shortRev(list.revisions.implementation_revision))
    expect(revs).toContain('abb46f1005+dirty.') // the dirty marker stays visible: a dirty tree is a different revision
  })

  it('lists every requirement with its state as a label and a symbol, not colour alone', async () => {
    const w = await mountView()
    expect(w.findAll('tbody tr')).toHaveLength(8)
    const s = t(w, 'state-KR-A06')
    expect(s.text()).toContain('result stale')
    expect(s.text()).toContain('⟳')
    expect(s.classes()).toContain('st-stale')
    expect(t(w, 'count').text()).toContain('8 of 8')
  })

  it('counts requirements per state in the chips', async () => {
    const w = await mountView()
    expect(t(w, 'chip-stale').text()).toContain('8')
    expect(t(w, 'chip-verified').text()).toContain('0')
    expect(w.findAll('[data-testid^="chip-"]')).toHaveLength(5)
  })

  it('keeps "satisfied · untested" and "verified" visibly different', async () => {
    const mixed = clone(list)
    mixed.requirements[0].assurance = 'verified'
    mixed.requirements[1].assurance = 'satisfied_untested'
    const w = await mountView(fakeClient({ list: vi.fn(async () => mixed) }))
    const a = t(w, 'state-KR-A01')
    const b = t(w, 'state-KR-A02')
    expect(a.text()).not.toBe(b.text())
    expect(a.text()).toContain('✓ verified')
    expect(b.text()).toContain('satisfied · untested')
    expect(a.classes()).not.toEqual(b.classes())
  })

  it('shows the source kind so binding obligations, policy and guidance stay distinguishable', async () => {
    const w = await mountView()
    expect(t(w, 'row-KR-A06').text()).toContain('policy')
  })

  it('lists each requirement\'s gaps', async () => {
    const w = await mountView()
    expect(t(w, 'row-KR-A01').find('.gaps').text()).toContain('stale')
  })
})

describe('filters are applied by the server, so only what is needed is loaded', () => {
  it('a state chip asks the server for that state only', async () => {
    const c = fakeClient()
    const w = await mountView(c)
    await t(w, 'chip-verified').trigger('click')
    await flushPromises()
    expect(c.list).toHaveBeenLastCalledWith({ state: 'verified', status: '', owner: '', gap: '' })
    expect(t(w, 'empty').exists()).toBe(true)
    expect(t(w, 'count').text()).toContain('0 of 8')
    expect(t(w, 'count').text()).toContain('filtered on the server')
    expect(t(w, 'chip-verified').attributes('aria-pressed')).toBe('true')
  })

  it('clicking the active chip again clears it', async () => {
    const c = fakeClient()
    const w = await mountView(c)
    await t(w, 'chip-stale').trigger('click')
    await flushPromises()
    expect(w.findAll('tbody tr')).toHaveLength(8)
    await t(w, 'chip-stale').trigger('click')
    await flushPromises()
    expect(t(w, 'chip-stale').attributes('aria-pressed')).toBe('false')
    expect(t(w, 'count').text()).not.toContain('filtered')
  })

  it('offers only values that exist, and filters by them', async () => {
    const c = fakeClient()
    const w = await mountView(c)
    const owners = t(w, 'filter-owner').findAll('option').map((o) => o.text())
    expect(owners).toEqual(['any', 'kr0ki-maintainers'])
    await t(w, 'filter-gap').setValue('stale')
    await flushPromises()
    expect(c.list).toHaveBeenLastCalledWith(expect.objectContaining({ gap: 'stale' }))
    await t(w, 'clear-filters').trigger('click')
    await flushPromises()
    expect(t(w, 'clear-filters').attributes('disabled')).toBeDefined()
  })
})

describe('the detail drawer shows the whole thread', () => {
  async function open() {
    const c = fakeClient()
    const w = await mountView(c)
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    return { w, c }
  }

  it('loads the requirement, its trace and its evidence', async () => {
    const { c } = await open()
    expect(c.requirement).toHaveBeenCalledWith('KR-A06')
    expect(c.trace).toHaveBeenCalledWith('KR-A06')
    expect(c.evidence).toHaveBeenCalledWith({ requirement: 'KR-A06' })
  })

  it('shows the statement, source obligations with their kind, element, control and case', async () => {
    const { w } = await open()
    expect(t(w, 'statement').text()).toContain('tool gateway shall deny')
    const obligations = t(w, 'obligations').text()
    expect(obligations).toContain('policy')
    expect(obligations).toContain('guidance') // the MCP security text
    expect(t(w, 'elements').text()).toContain('KrOKiAssurance::ToolGateway')
    expect(t(w, 'controls').text()).toContain('CTL-A06')
    expect(t(w, 'controls').text()).toContain('crates/kr0ki-server/src/gateway.rs')
    expect(t(w, 'cases').text()).toContain('VC-A06')
    expect(t(w, 'cases').text()).toContain('cargo test -p kr0ki-server --test assurance_a06')
  })

  it('shows evidence as revision-bound and says which revision moved when it is stale', async () => {
    const { w } = await open()
    const ev = t(w, 'evidence').text()
    expect(ev).toContain('pass')
    expect(ev).toContain('stale')
    expect(ev).toContain('implementation changed')
    expect(ev).toContain('artifact intact')
    expect(t(w, 'stale-VC-A06').exists()).toBe(true)
  })

  it('shows links as resolved at the current model revision', async () => {
    const { w } = await open()
    expect(t(w, 'links').text()).toContain('KrOKiAssurance::ToolGateway')
    expect(t(w, 'links').text()).toContain('resolved')
  })

  it('flags a control with no recorded implementation as a gap, not a blank', async () => {
    const d = clone(detailA06)
    d.controls[0].implementation = ''
    const w = await mountView(fakeClient({ requirement: vi.fn(async () => d) }))
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'no-impl').text()).toContain('implementation gap')
  })

  it('says plainly when nothing has ever been run', async () => {
    const d = clone(detailA06)
    d.thread.evidence = []
    d.thread.assurance = 'satisfied_untested'
    const w = await mountView(fakeClient({ requirement: vi.fn(async () => d) }))
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'no-evidence').text()).toContain('never been run')
    expect(t(w, 'detail-state').text()).toContain('satisfied · untested')
  })

  it('reports dangling links with the reason', async () => {
    const tr = clone(traceA06)
    tr.links[0] = { ...tr.links[0], status: 'dangling', reason: '`X` does not exist at model revision sha256:aa' }
    const w = await mountView(fakeClient({ trace: vi.fn(async () => tr) }))
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'links').text()).toContain('dangling')
    expect(t(w, 'links').text()).toContain('does not exist at model revision')
  })

  it('shows statement lint findings', async () => {
    const d = clone(detailA06)
    d.statement_lints = ['vague or open term `TBD`']
    const w = await mountView(fakeClient({ requirement: vi.fn(async () => d) }))
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'lints').text()).toContain('TBD')
  })

  it('closes, and a failing detail load does not break the list', async () => {
    const c = fakeClient({ requirement: vi.fn(async () => { throw refusal(404, 'not_found', 'unknown requirement `KR-A06`') }) })
    const w = await mountView(c)
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'detail-error').exists()).toBe(true)
    expect(w.findAll('tbody tr')).toHaveLength(8)
    await t(w, 'close-detail').trigger('click')
    expect(t(w, 'detail').exists()).toBe(false)
  })

  it('discards a slow answer for a requirement the person has already moved away from', async () => {
    let releaseFirst
    const slow = new Promise((r) => { releaseFirst = r })
    const first = clone(detailA06)
    const second = clone(detailA06)
    second.requirement.id = 'KR-A01'
    second.requirement.statement = 'The requirement adapter shall preserve identifiers.'
    const requirement = vi.fn((id) => (id === 'KR-A06' ? slow.then(() => first) : Promise.resolve(second)))
    const w = await mountView(fakeClient({ requirement }))
    await t(w, 'open-KR-A06').trigger('click')
    await t(w, 'open-KR-A01').trigger('click')
    await flushPromises()
    releaseFirst()
    await flushPromises()
    expect(t(w, 'statement').text()).toContain('requirement adapter')
  })
})

describe('running a case is explicit', () => {
  async function open(client = fakeClient()) {
    const w = await mountView(client)
    await t(w, 'open-KR-A06').trigger('click')
    await flushPromises()
    return w
  }

  it('asks first, naming what will run and where, and runs nothing until confirmed', async () => {
    const c = fakeClient()
    const w = await open(c)
    await t(w, 'run-VC-A06').trigger('click')
    expect(c.verify).not.toHaveBeenCalled()
    expect(t(w, 'cases').text()).toContain('runs the command above on the server')
    expect(t(w, 'confirm-run-VC-A06').exists()).toBe(true)
  })

  it('cancel runs nothing', async () => {
    const c = fakeClient()
    const w = await open(c)
    await t(w, 'run-VC-A06').trigger('click')
    await w.findAll('button').find((b) => b.text() === 'Cancel').trigger('click')
    expect(c.verify).not.toHaveBeenCalled()
    expect(t(w, 'run-VC-A06').exists()).toBe(true)
  })

  it('confirming runs the case at the implementation revision the view is showing, then refreshes', async () => {
    const c = fakeClient()
    const w = await open(c)
    const listCalls = c.list.mock.calls.length
    await t(w, 'run-VC-A06').trigger('click')
    await t(w, 'confirm-run-VC-A06').trigger('click')
    await flushPromises()
    expect(c.verify).toHaveBeenCalledWith('VC-A06', detailA06.revisions.implementation_revision)
    expect(t(w, 'run-notice').text()).toContain('VC-A06: pass')
    expect(t(w, 'run-notice').classes()).toContain('ok')
    expect(c.list.mock.calls.length).toBeGreaterThan(listCalls) // the view re-read the server
  })

  it('a failing case is shown as a failure, not a success', async () => {
    const c = fakeClient({ verify: vi.fn(async () => ({ case_id: 'VC-A06', result: 'fail', detail: '2 tests failed, 5 passed' })) })
    const w = await open(c)
    await t(w, 'run-VC-A06').trigger('click')
    await t(w, 'confirm-run-VC-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'run-notice').classes()).toContain('bad')
    expect(t(w, 'run-notice').text()).toContain('fail')
  })

  it('a refusal (the repository moved on) is explained and nothing is claimed', async () => {
    const c = fakeClient({ verify: vi.fn(async () => { throw refusal(409, 'revision_mismatch', 'you asked to verify X, the repository is at Y') }) })
    const w = await open(c)
    await t(w, 'run-VC-A06').trigger('click')
    await t(w, 'confirm-run-VC-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'run-notice').text()).toContain('moved on')
    expect(t(w, 'run-notice').classes()).toContain('bad')
  })

  it('a missing grant is explained', async () => {
    const c = fakeClient({ verify: vi.fn(async () => { throw refusal(403, 'forbidden', 'identity `reader` has no `assurance.verify` grant') }) })
    const w = await open(c)
    await t(w, 'run-VC-A06').trigger('click')
    await t(w, 'confirm-run-VC-A06').trigger('click')
    await flushPromises()
    expect(t(w, 'run-notice').text()).toContain('assurance.verify')
  })
})

describe('errors are explained, not swallowed', () => {
  const listFails = (e) => fakeClient({ list: vi.fn(async () => { throw e }) })

  it('401: tells the person to add a token under Setup', async () => {
    const w = await mountView(listFails(refusal(401, 'unauthorized', 'missing or invalid bearer token')))
    expect(t(w, 'error').text()).toContain('access token')
    expect(t(w, 'error').text()).toContain('Setup')
    expect(w.findAll('tbody tr')).toHaveLength(0)
  })

  it('403: names the grant that is missing', async () => {
    const w = await mountView(listFails(refusal(403, 'forbidden', 'identity `x` has no `assurance.read` grant')))
    expect(t(w, 'error').text()).toContain('assurance.read')
  })

  it('503: says the assurance thread is not enabled on this server', async () => {
    const w = await mountView(listFails(refusal(503, 'assurance_not_configured', 'x')))
    expect(t(w, 'error').text()).toContain('not enabled')
  })

  it('a network failure is reported as such', async () => {
    const w = await mountView(listFails(new TypeError('Failed to fetch')))
    expect(t(w, 'error').text()).toContain('Could not reach')
  })

  it('the error is announced to assistive technology', async () => {
    const w = await mountView(listFails(refusal(500, 'x', 'boom')))
    expect(t(w, 'error').attributes('role')).toBe('alert')
  })
})

describe('the diagram', () => {
  it('renders lazily, only when the tab is opened', async () => {
    const c = fakeClient()
    const w = await mountView(c)
    expect(c.viewSvg).not.toHaveBeenCalled()
    await t(w, 'a-tab-diagram').trigger('click')
    await flushPromises()
    expect(c.viewSvg).toHaveBeenCalledTimes(1)
    expect(t(w, 'diagram-img').attributes('src')).toBe('blob:fake-diagram')
    expect(t(w, 'diagram-img').attributes('alt')).toContain('Assurance thread diagram')
  })

  it('explains the two claims in its legend', async () => {
    const w = await mountView()
    await t(w, 'a-tab-diagram').trigger('click')
    await flushPromises()
    const legend = t(w, 'diagram-panel').text()
    expect(legend).toContain('satisfaction assertion')
    expect(legend).toContain('evidence')
  })

  it('shows a render failure instead of an empty box', async () => {
    const w = await mountView(fakeClient({ viewSvg: vi.fn(async () => { throw refusal(502, 'render_failed', 'backend down') }) }))
    await t(w, 'a-tab-diagram').trigger('click')
    await flushPromises()
    expect(t(w, 'diagram-error').text()).toContain('backend down')
    expect(t(w, 'diagram-img').exists()).toBe(false)
  })

  it('can be zoomed inside a scrolling frame, and opened at full size', async () => {
    const w = await mountView()
    await t(w, 'a-tab-diagram').trigger('click')
    await flushPromises()
    expect(t(w, 'diagram-img').attributes('style')).toContain('width: 100%')
    expect(t(w, 'zoom-100').attributes('aria-pressed')).toBe('true')
    await t(w, 'zoom-300').trigger('click')
    expect(t(w, 'diagram-img').attributes('style')).toContain('width: 300%')
    expect(t(w, 'zoom-300').attributes('aria-pressed')).toBe('true')
    expect(t(w, 'zoom-100').attributes('aria-pressed')).toBe('false')
    expect(w.find('.frame').attributes('tabindex')).toBe('0') // keyboard-scrollable
    expect(t(w, 'open-full').attributes('href')).toBe('blob:fake-diagram')
    expect(t(w, 'open-full').attributes('rel')).toBe('noopener')
  })

  it('releases the object URL on unmount', async () => {
    const w = await mountView()
    await t(w, 'a-tab-diagram').trigger('click')
    await flushPromises()
    w.unmount()
    expect(globalThis.URL.revokeObjectURL).toHaveBeenCalledWith('blob:fake-diagram')
  })
})

describe('the audit panel', () => {
  async function openAudit(client = fakeClient()) {
    const w = await mountView(client)
    await t(w, 'a-tab-audit').trigger('click')
    await flushPromises()
    return w
  }

  it('shows the records and says the chain verifies', async () => {
    const c = fakeClient()
    const w = await openAudit(c)
    expect(c.audit).toHaveBeenCalledWith({ decision: '', caller: '', phase: 'decision' })
    expect(t(w, 'chain').text()).toContain('Hash chain verifies')
    expect(t(w, 'chain').text()).toContain('17 record(s)')
    expect(w.findAll('[data-testid="audit-table"] tbody tr').length).toBe(audit.records.length)
  })

  it('shows a denial as a denial, with the reason', async () => {
    const w = await openAudit()
    const text = t(w, 'audit-table').text()
    expect(text).toContain('deny')
    expect(text).toContain('model.commit')
  })

  it('says loudly when the chain is broken', async () => {
    const broken = clone(audit)
    broken.chain = { ok: false, line: 4, seq: 3, reason: 'prev_hash does not match the previous record' }
    const w = await openAudit(fakeClient({ audit: vi.fn(async () => broken) }))
    expect(t(w, 'chain').text()).toContain('BROKEN at line 4')
    expect(t(w, 'chain').classes()).toContain('bad')
  })

  it('filters by decision on the server', async () => {
    const c = fakeClient()
    const w = await openAudit(c)
    await t(w, 'audit-decision').setValue('deny')
    await flushPromises()
    expect(c.audit).toHaveBeenLastCalledWith({ decision: 'deny', caller: '', phase: 'decision' })
  })

  it('explains that reading the audit log needs its own grant', async () => {
    const w = await openAudit(fakeClient({ audit: vi.fn(async () => { throw refusal(403, 'forbidden', 'identity `reader` has no `audit.read` grant') }) }))
    expect(t(w, 'audit-error').text()).toContain('audit.read')
    expect(t(w, 'audit-table').exists()).toBe(false)
  })

  it('does not load the log until the tab is opened', async () => {
    const c = fakeClient()
    await mountView(c)
    expect(c.audit).not.toHaveBeenCalled()
  })
})

describe('credentials', () => {
  it('never renders the token anywhere in the page', async () => {
    const w = await mountView(fakeClient(), { token: 'tok-super-secret' })
    await t(w, 'open-KR-A06').trigger('click')
    await t(w, 'a-tab-audit').trigger('click')
    await flushPromises()
    expect(w.html()).not.toContain('tok-super-secret')
  })

  it('builds its own client from the renderer url and token when none is injected', async () => {
    const calls = []
    globalThis.fetch = vi.fn(async (url, init) => { calls.push({ url, init }); return { ok: true, status: 200, json: async () => ({ ...list }) } })
    const w = mount(AssuranceView, { props: { rendererUrl: 'http://k.test:8787', token: 'tok-abc' }, attachTo: document.body })
    await flushPromises()
    expect(calls[0].url).toBe('http://k.test:8787/assurance/requirements')
    expect(calls[0].init.headers.Authorization).toBe('Bearer tok-abc')
    expect(w.html()).not.toContain('tok-abc')
  })

  it('re-reads when the token changes', async () => {
    const c = fakeClient()
    const w = await mountView(c)
    const before = c.list.mock.calls.length
    await w.setProps({ token: 'tok-new' })
    await flushPromises()
    expect(c.list.mock.calls.length).toBeGreaterThan(before)
  })
})
