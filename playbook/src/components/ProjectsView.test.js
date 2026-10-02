import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ProjectsView from './ProjectsView.vue'
import { createProjectStore, QuotaError } from '../lib/projects.js'

function memoryStorage({ limit = Infinity } = {}) {
  const m = new Map()
  return {
    getItem: (k) => (m.has(k) ? structuredClone(m.get(k)) : null),
    set(k, v) { const size = [...m].reduce((n, [a, b]) => n + a.length + JSON.stringify(b).length, 0) + JSON.stringify(v).length; if (size > limit) { const e = new Error('quota'); e.name = 'QuotaExceededError'; throw e } m.set(k, structuredClone(v)) },
    remove: (k) => m.delete(k), getAllKeys: () => [...m.keys()],
  }
}
let tick = 0
const newStore = (opts) => createProjectStore({ storage: memoryStorage(opts), now: () => `2026-10-01T10:00:${String(tick++).padStart(2, '0')}Z`, uuid: () => `p${tick++}` })

async function mountView(store = newStore(), props = {}) {
  const w = mount(ProjectsView, { props: { store, rendererUrl: 'http://k.test:8787', ...props }, attachTo: document.body })
  await flushPromises()
  return w
}
const setText = async (w, testid, value) => { const el = w.find(`[data-testid="${testid}"]`); await el.setValue(value) }
async function createWith(w, name, filePath) {
  await setText(w, 'new-project-name', name)
  await w.find('[data-testid="create-project"]').trigger('click')
  await flushPromises()
  if (filePath) { await setText(w, 'new-file-path', filePath); await w.find('[data-testid="add-file"]').trigger('click'); await flushPromises() }
}
// replace the working text of the selected file (the editor emits update:modelValue)
const edit = async (w, text) => { w.findComponent({ name: 'CodeEditor' }).vm.$emit('update:modelValue', text); await flushPromises() }
const save = async (w, msg) => { await setText(w, 'commit-message', msg); await w.find('[data-testid="save"]').trigger('submit'); await flushPromises() }

const REQS = `package V {
  requirement <'REQ-1'> mass { doc /* Mass <= 1000 kg */ attribute cost = 10; }
  requirement <'REQ-2'> range :> mass { doc /* Range >= 400 km */ attribute cost = 20; }
  part vehicle;
  satisfy mass by vehicle;
}`

const REQS_ATTR = `package V {
  requirement <'REQ-1'> mass {
    doc /* Mass <= 1000 kg */
    @CostAttribution { code = 'CC-4410/WBS-2.3'; share = 0.6; }
    @CostAttribution { code = 'CC-9000'; share = 0.4; }
    @ComplianceTag { tag = 'au-ai6:P5'; basis = 'judgement'; }
  }
  requirement <'REQ-2'> range { doc /* Range >= 400 km */ @ComplianceTag { tag = 'au-ai6:P9'; } }
  requirement <'REQ-3'> third {
    @CostAttribution { code = 'CC-1'; share = 0.7; }
    @CostAttribution { code = 'CC-2'; share = 0.7; }
    @CostAttribution { code = 'bad code!'; share = 0.1; }
    @ComplianceTag { tag = 'iso42001:A.6.2.6'; }
  }
  part vehicle;
  satisfy mass by vehicle;
}`

beforeEach(() => { tick = 0; vi.stubGlobal('confirm', () => true); vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: () => 'blob:x', revokeObjectURL() {} })) })

describe('ProjectsView', () => {
  it('starts empty with an explanation, and creating a project opens it', async () => {
    const w = await mountView()
    expect(w.find('[data-testid="projects-empty"]').text()).toContain('every save recorded as a commit')
    await createWith(w, 'Vehicle')
    expect(w.find('[data-testid="projects-empty"]').exists()).toBe(false)
    expect(w.find('[data-testid="project-select"]').element.value).not.toBe('')
    expect(w.find('[data-testid="notice"]').text()).toContain('Created project "Vehicle"')
  })

  it('adds a SysML file from a template, finds its requirement live, and saves a commit that names what it affected', async () => {
    const w = await mountView()
    await createWith(w, 'Vehicle', 'model/vehicle.sysml')
    await w.find('[data-testid="tab-requirements"]').trigger('click')
    expect(w.find('[data-testid="req-REQ-1"]').exists()).toBe(true) // from the template, before any save
    await save(w, 'first requirement')
    expect(w.find('[data-testid="notice"]').text()).toMatch(/Saved \w{8}: 1 file changed, 1 requirement affected \(REQ-1\)/)
    expect(w.find('[data-testid="tab-history"]').text()).toContain('History (1)')
    expect(w.find('[data-testid="panel-history"]').text()).toContain('first requirement')
    expect(w.find('[data-testid="panel-history"]').text()).toContain('affects')
    await save(w, 'again')
    expect(w.find('[data-testid="notice"]').text()).toContain('Nothing changed')
  })

  it('refuses to save without a message and shows why', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'a.sysml')
    await save(w, '   ')
    expect(w.find('[data-testid="notice"]').attributes('data-kind')).toBe('error')
    expect(w.find('[data-testid="notice"]').text()).toContain('commit needs a message')
  })

  it('shows who each requirement is charged to, flags gaps, and keeps the old numeric weight only as a legacy line', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml')
    await edit(w, REQS_ATTR)
    await w.find('[data-testid="tab-requirements"]').trigger('click')
    expect(w.find('[data-testid="req-REQ-1"] [data-col="attribution"]').text()).toContain('CC-4410/WBS-2.3 · 60%')
    expect(w.find('[data-testid="req-REQ-1"] [data-col="attribution"]').text()).toContain('CC-9000 · 40%')
    expect(w.find('[data-testid="req-REQ-2"] [data-col="attribution"]').text()).toContain('not charged')
    expect(w.find('[data-testid="req-REQ-1"] [data-col="tags"]').text()).toContain('au-ai6:P5')
    expect(w.find('[data-testid="gaps"]').text()).toContain('not yet fully covered')
    expect(w.find('[data-testid="scenario-total"]').exists()).toBe(false) // no numeric cost anywhere: no money-like total at all
    await edit(w, REQS) // the older projects with `attribute cost` still work
    expect(w.find('[data-testid="scenario-total"]').text()).toContain('30')
    expect(w.find('[data-testid="scenario-total"]').text()).toContain('not an accounting amount')
    await w.find('[data-testid="req-REQ-2"] input[type="checkbox"]').setValue(false)
    expect(w.find('[data-testid="scenario-total"]').text()).toContain('10')
  })

  it('summarises attribution by code and every parent, and says what is not charged, over-charged or unknown', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml')
    await edit(w, REQS_ATTR)
    await w.find('[data-testid="tab-attribution"]').trigger('click')
    const table = w.find('[data-testid="attribution-table"]').text()
    expect(table).toContain('CC-4410')
    expect(table).toContain('CC-4410/WBS-2.3')
    expect(table).toContain('CC-9000')
    const issues = w.find('[data-testid="attribution-issues"]').text()
    expect(issues).toContain('Not charged to any code: REQ-2')
    expect(issues).toContain('REQ-3: shares add up to 1.5')
    expect(issues).toContain('Not a valid code: bad code!')
    await w.find('[data-testid="codebook"]').trigger('click'); await flushPromises() // creates attribution/codebook.json
    expect(w.find('.projects__files').text()).toContain('attribution/codebook.json')
    await w.find('[data-testid="tab-attribution"]').trigger('click')
    expect(w.find('[data-testid="attribution-issues"]').text()).toContain('Not in the code book: CC-4410/WBS-2.3')
  })

  it('adds an attribution code or compliance tag to a requirement by form, validates them, and saves a link-only change', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml')
    await edit(w, REQS)
    await save(w, 'baseline')
    await w.find('[data-testid="tab-requirements"]').trigger('click')
    const add = async (kind, value, extra = {}) => {
      await w.find('[data-testid="link-requirement"]').setValue('REQ-1')
      await w.find('[data-testid="link-kind"]').setValue(kind)
      await w.find('[data-testid="link-value"]').setValue(value)
      if (extra.share !== undefined) await w.find('[data-testid="link-share"]').setValue(extra.share)
      await w.find('[data-testid="add-link"]').trigger('submit'); await flushPromises()
    }
    await add('attribution', 'CC 4410')
    expect(w.find('[data-testid="notice"]').text()).toContain('not a valid attribution code')
    await add('attribution', 'CC-4410/WBS-2.3', { share: 2 })
    expect(w.find('[data-testid="notice"]').text()).toContain('share must be above 0')
    await add('attribution', 'CC-4410/WBS-2.3', { share: 0.75 })
    expect(w.find('[data-testid="req-REQ-1"] [data-col="attribution"]').text()).toContain('CC-4410/WBS-2.3 · 75%')
    await add('tag', 'not a tag')
    expect(w.find('[data-testid="notice"]').text()).toContain('is not a tag')
    await add('tag', 'au-ai6:P5')
    expect(w.find('[data-testid="req-REQ-1"] [data-col="tags"]').text()).toContain('au-ai6:P5')
    await save(w, 'charge REQ-1 and tag it')
    expect(w.find('[data-testid="notice"]').text()).toContain('0 files changed, 1 requirement affected (REQ-1)') // no file changed: the links are versioned too
    await w.find('[data-testid="tab-history"]').trigger('click')
    expect(w.find('[data-testid="panel-history"]').text()).toContain('charge REQ-1 and tag it')
  })

  it('the share field accepts every fraction a user can sensibly type (the browser blocks the whole form otherwise)', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml'); await edit(w, REQS)
    await w.find('[data-testid="tab-requirements"]').trigger('click')
    const share = w.find('[data-testid="link-share"]')
    for (const v of ['1', '0.75', '0.6', '0.05', '0.333', '0.5']) { await share.setValue(v); expect(share.element.checkValidity(), v).toBe(true) }
  })

  it('shows compliance coverage per framework with provenance, gaps, and malformed or unknown tags', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml')
    await edit(w, REQS_ATTR)
    await w.find('[data-testid="tab-compliance"]').trigger('click')
    expect(w.find('[data-testid="panel-compliance"]').text()).toContain('Working aid, not legal advice')
    const p5 = w.find('[data-testid="ctl-P5"]').text()
    expect(p5).toContain('REQ-1'); expect(p5).toContain('judgement')
    expect(w.find('[data-testid="ctl-P1"]').text()).toContain('no requirement yet')
    expect(w.find('[data-testid="coverage-summary"]').text()).toMatch(/1 of 6 controls/)
    expect(w.find('[data-testid="panel-compliance"]').text()).toContain('au-ai6:P9')
    await w.find('[data-testid="framework-select"]').setValue('iso42001')
    expect(w.find('[data-testid="ctl-A.6"]').text()).toContain('REQ-3') // iso42001:A.6.2.6 counts toward A.6
    expect(w.find('[data-testid="panel-compliance"]').text()).toContain('recalled')
  })

  it('warns when the same requirement id is declared in two files', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'a.sysml')
    await setText(w, 'new-file-path', 'b.sysml'); await w.find('[data-testid="add-file"]').trigger('click'); await flushPromises() // both templates declare REQ-1
    await w.find('[data-testid="tab-requirements"]').trigger('click')
    expect(w.find('[data-testid="duplicates"]').text()).toContain('REQ-1')
    expect(w.find('[data-testid="duplicates"]').text()).toContain('a.sysml, b.sysml')
  })

  it('a diagram linked as depicting a requirement makes editing it count as touching that requirement', async () => {
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml')
    await edit(w, REQS)
    await setText(w, 'new-file-path', 'diagrams/context.d2'); await w.find('[data-testid="add-file"]').trigger('click'); await flushPromises()
    await setText(w, 'depicts-id', 'REQ-2'); await w.find('[data-testid="add-depicts"]').trigger('click'); await flushPromises()
    await save(w, 'initial')
    await edit(w, 'a -> b -> c')
    await save(w, 'rework the diagram')
    expect(w.find('[data-testid="notice"]').text()).toContain('1 requirement affected (REQ-2)')
  })

  it('keeps unsaved work across a reload and reloads a past version on request', async () => {
    const store = newStore()
    const w = await mountView(store)
    await createWith(w, 'V', 'v.sysml')
    await edit(w, REQS)
    await save(w, 'v1')
    await edit(w, REQS + '\n// wip')
    await new Promise((r) => setTimeout(r, 400)) // the draft is saved after a short pause
    w.unmount()
    const again = await mountView(store)
    expect(again.findComponent({ name: 'CodeEditor' }).props('modelValue')).toContain('// wip')
    expect(again.find('.projects__files').text()).toContain('1 unsaved')
    await again.findAll('.commits button.link').find((b) => b.text() === 'Load this version').trigger('click')
    expect(again.findComponent({ name: 'CodeEditor' }).props('modelValue')).not.toContain('// wip')
  })

  it('imports a ReqIF file through the server, adds requirements as SysML text and keeps relations as traces', async () => {
    const imported = { documents: [{ graph: { requirements: [{ id: 'REQ-1', title: 'Encrypt', text: 'Encrypt at rest.' }, { id: 'REQ-2', title: 'Rotate', text: 'Rotate keys.' }], relations: [{ source: 'REQ-2', target: 'REQ-1', kind: 'derives' }] } }] }
    const calls = []
    vi.stubGlobal('fetch', vi.fn(async (url, init) => { calls.push([String(url), init?.method]); return { ok: true, status: 200, json: async () => imported } }))
    const w = await mountView()
    await createWith(w, 'Needs')
    const input = w.find('[data-testid="reqif-input"]')
    const file = new File(['<REQ-IF/>'], 'needs.reqif', { type: 'application/xml' })
    Object.defineProperty(input.element, 'files', { value: [file] })
    await input.trigger('change'); await flushPromises()
    expect(calls).toEqual([['http://k.test:8787/requirements/import', 'POST']])
    expect(w.find('[data-testid="notice"]').text()).toContain('Imported 2 requirements')
    expect(w.find('.projects__files').text()).toContain('requirements/needs.sysml')
    expect(w.find('.projects__files').text()).toContain('requirements/needs.reqif')
    await w.find('[data-testid="tab-requirements"]').trigger('click')
    expect(w.find('[data-testid="req-REQ-1"]').exists() && w.find('[data-testid="req-REQ-2"]').exists()).toBe(true)
    await save(w, 'import needs')
    expect(w.find('[data-testid="notice"]').text()).toContain('2 requirements affected')
  })

  it('rejects .reqifz and failed imports with a clear message', async () => {
    const w = await mountView()
    await createWith(w, 'Needs')
    const input = w.find('[data-testid="reqif-input"]')
    Object.defineProperty(input.element, 'files', { value: [new File(['x'], 'bundle.reqifz')], configurable: true })
    await input.trigger('change'); await flushPromises()
    expect(w.find('[data-testid="notice"]').text()).toContain('.reqifz')
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 422, json: async () => ({ message: 'missing <REQ-IF>' }) })))
    Object.defineProperty(input.element, 'files', { value: [new File(['x'], 'bad.reqif')], configurable: true })
    await input.trigger('change'); await flushPromises()
    expect(w.find('[data-testid="notice"]').text()).toContain('missing <REQ-IF>')
  })

  it('draws the requirement graph by posting D2 to the renderer', async () => {
    let posted = null
    vi.stubGlobal('fetch', vi.fn(async (url, init) => { posted = [String(url), init.body]; return { ok: true, blob: async () => new Blob(['<svg/>']) } }))
    const w = await mountView()
    await createWith(w, 'V', 'v.sysml'); await edit(w, REQS)
    await w.find('[data-testid="tab-graph"]').trigger('click')
    await w.find('[data-testid="draw-graph"]').trigger('click'); await flushPromises()
    expect(posted[0]).toBe('http://k.test:8787/render/d2?output=svg')
    expect(posted[1]).toContain('"REQ-2" -> "REQ-1": "derive"')
    expect(w.find('[data-testid="graph-image"]').exists()).toBe(true)
  })

  it('tells the user when the browser storage is full instead of losing the save', async () => {
    const w = await mountView(newStore({ limit: 1800 }))
    await createWith(w, 'Tight', 'big.sysml')
    await edit(w, "package P { requirement <'R'> r { doc /* " + 'x'.repeat(3000) + ' */ } }')
    await save(w, 'too big')
    expect(w.find('[data-testid="notice"]').attributes('data-kind')).toBe('error')
    expect(w.find('[data-testid="notice"]').text()).toContain('storage is full')
    expect(QuotaError).toBeTypeOf('function')
  })
})
