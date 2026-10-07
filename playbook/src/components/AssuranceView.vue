<script setup>
// The assurance thread: source obligation -> requirement -> system element -> enforcing control -> verification case -> evidence.
// Every number and state comes from the server's /assurance/* routes, qualified by the revisions it was computed at.
// This view keeps two claims apart: a satisfaction ASSERTION (architectural) and a verification RESULT (revision-bound evidence).
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import {
  STATES, createAssuranceClient, describeGap, driftText, explainError, gapKind, shortRev, sourceKindInfo, stateInfo,
} from '../lib/assurance.js'

const props = defineProps({
  rendererUrl: { type: String, default: '' },
  token: { type: String, default: '' }, // kept for the browser session by App; only ever sent as a bearer header
  client: { type: Object, default: null }, // injected in tests
})

const api = computed(() => props.client || createAssuranceClient({ baseUrl: props.rendererUrl || window.location.origin, token: props.token }))

const tab = ref('requirements')
const loading = ref(false)
const error = ref('')
const all = ref(null) // unfiltered list: the summary chips and the filter options
const list = ref(null) // what the table shows (filtered by the server when a filter is set)
const filters = reactive({ state: '', status: '', owner: '', gap: '' })

const selectedId = ref('')
const detail = ref(null)
const trace = ref(null)
const evidence = ref([])
const detailBusy = ref(false)
const detailError = ref('')

const diagramUrl = ref('')
const diagramBusy = ref(false)
const diagramZoom = ref(100) // percent of the frame's width; the SVG has only a viewBox, so zoom is relative to the frame
const diagramError = ref('')

const audit = ref(null)
const auditBusy = ref(false)
const auditError = ref('')
const auditFilter = reactive({ decision: '', caller: '' })

const confirmingCase = ref('')
const runBusy = ref(false)
const runNotice = ref(null) // { kind: 'ok'|'error', text }

const hasFilter = computed(() => Object.values(filters).some(Boolean))
const revisions = computed(() => (all.value || list.value || {}).revisions || null)
const counts = computed(() => {
  const c = Object.fromEntries(STATES.map((s) => [s.id, 0]))
  for (const r of (all.value?.requirements || [])) c[r.assurance] = (c[r.assurance] || 0) + 1
  return c
})
const options = computed(() => {
  const rows = all.value?.requirements || []
  const uniq = (xs) => [...new Set(xs.filter(Boolean))].sort()
  return {
    status: uniq(rows.map((r) => r.status)),
    owner: uniq(rows.map((r) => r.owner)),
    gap: uniq(rows.flatMap((r) => r.gaps.map(gapKind))),
  }
})
const rows = computed(() => list.value?.requirements || [])

async function load() {
  loading.value = true
  error.value = ''
  try {
    all.value = await api.value.list()
    list.value = hasFilter.value ? await api.value.list({ ...filters }) : all.value
  } catch (e) {
    error.value = explainError(e)
    all.value = null
    list.value = null
  } finally {
    loading.value = false
  }
}

async function applyFilters() {
  if (!all.value) return load()
  error.value = ''
  try {
    list.value = hasFilter.value ? await api.value.list({ ...filters }) : all.value
  } catch (e) {
    error.value = explainError(e)
  }
}

function clearFilters() {
  for (const k of Object.keys(filters)) filters[k] = ''
  applyFilters()
}

function chip(stateId) {
  filters.state = filters.state === stateId ? '' : stateId
  applyFilters()
}

async function select(id) {
  selectedId.value = id
  detail.value = null
  trace.value = null
  evidence.value = []
  detailError.value = ''
  runNotice.value = null
  confirmingCase.value = ''
  detailBusy.value = true
  try {
    const [d, t, ev] = await Promise.all([api.value.requirement(id), api.value.trace(id), api.value.evidence({ requirement: id })])
    if (selectedId.value !== id) return // the person moved on
    detail.value = d
    trace.value = t
    evidence.value = ev.records
  } catch (e) {
    if (selectedId.value === id) detailError.value = explainError(e)
  } finally {
    if (selectedId.value === id) detailBusy.value = false
  }
}

function closeDetail() {
  selectedId.value = ''
  detail.value = null
}

function revokeDiagram() {
  if (diagramUrl.value && globalThis.URL?.revokeObjectURL) globalThis.URL.revokeObjectURL(diagramUrl.value)
  diagramUrl.value = ''
}

async function loadDiagram() {
  diagramBusy.value = true
  diagramError.value = ''
  try {
    const blob = await api.value.viewSvg()
    revokeDiagram()
    diagramUrl.value = globalThis.URL.createObjectURL(blob)
  } catch (e) {
    diagramError.value = explainError(e)
  } finally {
    diagramBusy.value = false
  }
}

async function loadAudit() {
  auditBusy.value = true
  auditError.value = ''
  try {
    audit.value = await api.value.audit({ ...auditFilter, phase: 'decision' })
  } catch (e) {
    audit.value = null
    auditError.value = explainError(e)
  } finally {
    auditBusy.value = false
  }
}

async function runCase(caseId) {
  if (!detail.value) return
  runBusy.value = true
  runNotice.value = null
  try {
    const out = await api.value.verify(caseId, detail.value.revisions.implementation_revision)
    runNotice.value = { kind: out.result === 'pass' ? 'ok' : 'error', text: `${caseId}: ${out.result} — ${out.detail}` }
    confirmingCase.value = ''
    const id = selectedId.value
    await load()
    await select(id)
    runNotice.value = { kind: out.result === 'pass' ? 'ok' : 'error', text: `${caseId}: ${out.result} — ${out.detail}` }
  } catch (e) {
    runNotice.value = { kind: 'error', text: explainError(e) }
  } finally {
    runBusy.value = false
  }
}

watch(tab, (t) => {
  if (t === 'diagram' && !diagramUrl.value) loadDiagram()
  if (t === 'audit' && !audit.value) loadAudit()
})
watch(() => [props.rendererUrl, props.token], () => { revokeDiagram(); audit.value = null; load() })
onMounted(load)
onBeforeUnmount(revokeDiagram)

const linkClass = (l) => (l.status === 'resolved' ? 'ok' : 'bad')
const resultClass = (r) => (r === 'pass' ? 'ok' : 'bad')
const evidenceFor = (key) => evidence.value.find((e) => e.evidence_key === key)
</script>

<template>
  <section class="assurance" data-testid="assurance-view">
    <header class="a-head">
      <div>
        <p class="eyebrow">Assurance thread</p>
        <h2>Obligation → requirement → element → control → case → evidence</h2>
        <p class="lede">
          <b>Satisfied</b> is an architectural assertion. <b>Verified</b> is evidence that a case passed at the current model, implementation and
          configuration revisions: change any one and it goes <b>stale</b>.
        </p>
      </div>
      <div class="a-revs" data-testid="revisions" v-if="revisions">
        <span>model <code :title="revisions.model_revision">{{ shortRev(revisions.model_revision) }}</code></span>
        <span>implementation <code :title="revisions.implementation_revision">{{ shortRev(revisions.implementation_revision) }}</code></span>
        <button type="button" class="ghost" data-testid="refresh" :disabled="loading" @click="load">{{ loading ? 'Loading…' : 'Refresh' }}</button>
      </div>
    </header>

    <p v-if="error" class="notice bad" role="alert" data-testid="error">{{ error }}</p>

    <nav class="a-tabs" aria-label="Assurance panels">
      <button type="button" :class="{ active: tab === 'requirements' }" data-testid="a-tab-requirements" @click="tab = 'requirements'">Requirements</button>
      <button type="button" :class="{ active: tab === 'diagram' }" data-testid="a-tab-diagram" @click="tab = 'diagram'">Diagram</button>
      <button type="button" :class="{ active: tab === 'audit' }" data-testid="a-tab-audit" @click="tab = 'audit'">Audit</button>
    </nav>

    <!-- ── requirements ─────────────────────────────────────────────────── -->
    <div v-if="tab === 'requirements'" class="a-body" :class="{ withDetail: selectedId }">
      <div class="a-main">
        <ul class="chips" aria-label="Requirements by assurance state" data-testid="chips">
          <li v-for="s in STATES" :key="s.id">
            <button
              type="button" class="chip" :class="[`st-${s.id}`, { on: filters.state === s.id }]" :title="s.hint"
              :aria-pressed="filters.state === s.id" :data-testid="`chip-${s.id}`" @click="chip(s.id)"
            >
              <span aria-hidden="true">{{ s.symbol }}</span> {{ s.label }} <b>{{ counts[s.id] }}</b>
            </button>
          </li>
        </ul>

        <form class="filters" data-testid="filters" @submit.prevent="applyFilters">
          <label>Status
            <select v-model="filters.status" data-testid="filter-status" @change="applyFilters">
              <option value="">any</option><option v-for="o in options.status" :key="o">{{ o }}</option>
            </select>
          </label>
          <label>Owner
            <select v-model="filters.owner" data-testid="filter-owner" @change="applyFilters">
              <option value="">any</option><option v-for="o in options.owner" :key="o">{{ o }}</option>
            </select>
          </label>
          <label>Gap
            <select v-model="filters.gap" data-testid="filter-gap" @change="applyFilters">
              <option value="">any</option><option v-for="o in options.gap" :key="o">{{ o }}</option>
            </select>
          </label>
          <button type="button" class="ghost" data-testid="clear-filters" :disabled="!hasFilter" @click="clearFilters">Clear</button>
        </form>

        <p class="count" data-testid="count" aria-live="polite">
          <template v-if="list">{{ list.total }} of {{ all?.total ?? list.total }} requirement(s)<span v-if="hasFilter"> — filtered on the server, only these were loaded</span></template>
          <template v-else-if="loading">Loading…</template>
        </p>

        <table v-if="rows.length" class="req-table" data-testid="table">
          <thead>
            <tr>
              <th scope="col">Requirement</th>
              <th scope="col">Satisfaction <small>(asserted)</small></th>
              <th scope="col">State</th>
              <th scope="col">Gaps</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in rows" :key="r.id" :class="{ selected: r.id === selectedId }" :data-testid="`row-${r.id}`">
              <td>
                <button type="button" class="link" :data-testid="`open-${r.id}`" @click="select(r.id)"><b>{{ r.id }}</b> {{ r.title }}</button>
                <div class="sub">
                  <span class="badge" :title="sourceKindInfo(r.source_kind).hint">{{ sourceKindInfo(r.source_kind).label }}</span>
                  {{ r.status }} · {{ r.owner }}
                </div>
              </td>
              <td class="sat">{{ r.assurance === 'unsatisfied' ? 'none' : 'asserted' }}</td>
              <td>
                <span class="state" :class="`st-${r.assurance}`" :title="stateInfo(r.assurance).hint" :data-testid="`state-${r.id}`">
                  <span aria-hidden="true">{{ stateInfo(r.assurance).symbol }}</span> {{ stateInfo(r.assurance).label }}
                </span>
              </td>
              <td>
                <ul class="gaps"><li v-for="(g, i) in r.gaps" :key="i" :title="describeGap(g)">{{ gapKind(g) }}</li></ul>
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else-if="list && !loading && !error" class="empty" data-testid="empty">No requirement matches these filters.</p>
      </div>

      <!-- ── detail drawer ─────────────────────────────────────────────── -->
      <aside v-if="selectedId" class="detail" data-testid="detail" aria-label="Requirement detail">
        <button type="button" class="ghost close" data-testid="close-detail" @click="closeDetail">Close</button>
        <p v-if="detailBusy" class="muted">Loading {{ selectedId }}…</p>
        <p v-if="detailError" class="notice bad" role="alert" data-testid="detail-error">{{ detailError }}</p>

        <template v-if="detail">
          <h3>{{ detail.requirement.id }}
            <span class="state" :class="`st-${detail.thread.assurance}`" data-testid="detail-state">
              <span aria-hidden="true">{{ stateInfo(detail.thread.assurance).symbol }}</span> {{ stateInfo(detail.thread.assurance).label }}
            </span>
          </h3>
          <blockquote class="statement" data-testid="statement">{{ detail.requirement.statement }}</blockquote>
          <ul v-if="detail.statement_lints.length" class="lints" data-testid="lints">
            <li v-for="l in detail.statement_lints" :key="l">⚠ {{ l }}</li>
          </ul>
          <dl class="facts">
            <dt>Status</dt><dd>{{ detail.requirement.status }}</dd>
            <dt>Owner</dt><dd>{{ detail.requirement.owner }}</dd>
            <dt>Rationale</dt><dd>{{ detail.requirement.rationale }}</dd>
            <dt>Acceptance</dt><dd>{{ detail.requirement.acceptance }}</dd>
          </dl>

          <h4>1 · Source obligations</h4>
          <ul class="thread" data-testid="obligations">
            <li v-for="o in detail.obligations" :key="o.id">
              <span class="badge" :title="sourceKindInfo(o.source_kind).hint">{{ sourceKindInfo(o.source_kind).label }}</span>
              <b>{{ o.title }}</b> <small>{{ o.version }}</small><br><small class="muted">{{ o.source }}</small>
            </li>
            <li v-if="!detail.obligations.length" class="bad">No source obligation derives this requirement.</li>
          </ul>

          <h4>2 · System element <small>(satisfaction is asserted)</small></h4>
          <ul class="thread" data-testid="elements">
            <li v-for="e in detail.elements" :key="e"><code>{{ e }}</code></li>
            <li v-if="!detail.elements.length" class="bad">Nothing asserts that an element satisfies this.</li>
          </ul>

          <h4>3 · Enforcing control</h4>
          <ul class="thread" data-testid="controls">
            <li v-for="c in detail.controls" :key="c.id">
              <b>{{ c.id }}</b> {{ c.title }}
              <span class="badge">{{ c.component === 'deployment' ? 'deployment-enforced' : 'in kr0ki' }}</span><br>
              <code v-if="c.implementation" class="impl">{{ c.implementation }}</code>
              <span v-else class="bad" data-testid="no-impl">no implementation recorded: an implementation gap</span>
            </li>
            <li v-if="!detail.controls.length" class="bad">No control enforces this.</li>
          </ul>

          <h4>4 · Verification case</h4>
          <ul class="thread" data-testid="cases">
            <li v-for="c in detail.cases" :key="c.id">
              <b>{{ c.id }}</b> {{ c.title }}<br>
              <small class="muted">accepts: {{ c.acceptance }}</small><br>
              <code class="impl">{{ c.command.join(' ') }}</code>
              <div class="run">
                <button v-if="confirmingCase !== c.id" type="button" class="ghost" :data-testid="`run-${c.id}`" @click="confirmingCase = c.id">Run this case…</button>
                <template v-else>
                  <small>This runs the command above on the server, against revision <code>{{ shortRev(detail.revisions.implementation_revision) }}</code>, and stores evidence.</small>
                  <button type="button" class="go" :disabled="runBusy" :data-testid="`confirm-run-${c.id}`" @click="runCase(c.id)">{{ runBusy ? 'Running…' : 'Run it' }}</button>
                  <button type="button" class="ghost" :disabled="runBusy" @click="confirmingCase = ''">Cancel</button>
                </template>
              </div>
            </li>
            <li v-if="!detail.cases.length" class="bad">No verification case.</li>
          </ul>
          <p v-if="runNotice" class="notice" :class="runNotice.kind === 'ok' ? 'ok' : 'bad'" role="status" data-testid="run-notice">{{ runNotice.text }}</p>

          <h4>5 · Evidence <small>(revision-bound)</small></h4>
          <ul class="thread" data-testid="evidence">
            <li v-for="e in detail.thread.evidence" :key="e.key">
              <span class="state" :class="resultClass(e.result) === 'ok' ? 'st-verified' : 'st-failing'">{{ e.result }}</span>
              <span v-if="e.freshness.state === 'fresh'" class="ok">fresh</span>
              <span v-else class="stale" :data-testid="`stale-${e.verification_id}`">stale · {{ driftText(e.freshness) }}</span>
              <small class="muted"> {{ e.verification_id }}</small>
              <div v-if="evidenceFor(e.key)" class="sub">
                model <code>{{ shortRev(evidenceFor(e.key).record.model_revision) }}</code>
                impl <code>{{ shortRev(evidenceFor(e.key).record.implementation_revision) }}</code>
                · artifact <span :class="evidenceFor(e.key).artifact === 'intact' ? 'ok' : 'bad'">{{ evidenceFor(e.key).artifact }}</span>
              </div>
            </li>
            <li v-if="!detail.thread.evidence.length" class="muted" data-testid="no-evidence">No evidence: this has never been run.</li>
          </ul>

          <h4>Links <small>(resolved at the current model revision)</small></h4>
          <ul class="thread" v-if="trace" data-testid="links">
            <li v-for="(l, i) in trace.links" :key="i" :class="linkClass(l)">
              <span class="badge">{{ l.class.replace('_', ' ') }}</span> <code>{{ l.locator }}</code>
              <b>{{ l.status }}</b><span v-if="l.reason"> — {{ l.reason }}</span>
            </li>
          </ul>

          <h4 v-if="detail.gaps.length">Gaps</h4>
          <ul v-if="detail.gaps.length" class="thread" data-testid="detail-gaps">
            <li v-for="(g, i) in detail.gaps" :key="i"><b>{{ gapKind(g) }}</b>: {{ describeGap(g) }}</li>
          </ul>
        </template>
      </aside>
    </div>

    <!-- ── diagram ──────────────────────────────────────────────────────── -->
    <div v-else-if="tab === 'diagram'" class="a-diagram" data-testid="diagram-panel">
      <div class="legend" aria-label="Legend">
        <span class="chip st-verified">✓ verified</span><span class="chip st-satisfied_untested">◐ satisfied · untested</span>
        <span class="chip st-failing">✗ failing</span><span class="chip st-stale">⟳ result stale</span><span class="chip st-unsatisfied">○ not satisfied</span>
        <span class="muted">Dashed edge = <b>satisfaction assertion</b>. Cloud = <b>evidence</b> with result and freshness.</span>
        <button type="button" class="ghost" :disabled="diagramBusy" data-testid="reload-diagram" @click="loadDiagram">{{ diagramBusy ? 'Rendering…' : 'Re-render' }}</button>
      </div>
      <div v-if="diagramUrl" class="zoom" role="group" aria-label="Diagram zoom">
        <button v-for="z in [100, 200, 300]" :key="z" type="button" class="ghost" :class="{ on: diagramZoom === z }" :aria-pressed="diagramZoom === z" :data-testid="`zoom-${z}`" @click="diagramZoom = z">{{ z === 100 ? 'Fit' : `${z / 100}×` }}</button>
        <a :href="diagramUrl" target="_blank" rel="noopener" data-testid="open-full">Open full size ↗</a>
      </div>
      <p v-if="diagramError" class="notice bad" role="alert" data-testid="diagram-error">{{ diagramError }}</p>
      <p v-else-if="diagramBusy && !diagramUrl" class="muted">Rendering…</p>
      <div v-if="diagramUrl" class="frame" tabindex="0" aria-label="Diagram (scrollable)">
        <img :src="diagramUrl" alt="Assurance thread diagram: requirements, elements, controls, cases and evidence" class="diagram" :style="{ width: `${diagramZoom}%` }" data-testid="diagram-img">
      </div>
    </div>

    <!-- ── audit ────────────────────────────────────────────────────────── -->
    <div v-else class="a-audit" data-testid="audit-panel">
      <form class="filters" @submit.prevent="loadAudit">
        <label>Decision
          <select v-model="auditFilter.decision" data-testid="audit-decision" @change="loadAudit">
            <option value="">any</option><option value="permit">permit</option><option value="deny">deny</option>
          </select>
        </label>
        <label>Caller <input v-model="auditFilter.caller" data-testid="audit-caller" placeholder="identity id" @change="loadAudit"></label>
        <button type="button" class="ghost" :disabled="auditBusy" data-testid="reload-audit" @click="loadAudit">{{ auditBusy ? 'Loading…' : 'Reload' }}</button>
      </form>
      <p v-if="auditError" class="notice bad" role="alert" data-testid="audit-error">{{ auditError }}</p>
      <template v-if="audit">
        <p class="count" data-testid="chain" :class="audit.chain.ok ? 'ok' : 'bad'">
          <template v-if="audit.chain.ok">Hash chain verifies: {{ audit.chain.records }} record(s) in the log, none altered or removed.</template>
          <template v-else>Hash chain BROKEN at line {{ audit.chain.line }}: {{ audit.chain.reason }}</template>
        </p>
        <table class="req-table" data-testid="audit-table">
          <thead><tr><th>#</th><th>When</th><th>Caller</th><th>Operation</th><th>Decision</th><th>Model revision</th><th>Via</th><th>Correlation</th></tr></thead>
          <tbody>
            <tr v-for="r in audit.records" :key="r.seq" :class="r.decision">
              <td>{{ r.seq }}</td><td>{{ r.at }}</td><td>{{ r.caller }}</td><td>{{ r.operation }}</td>
              <td><span class="state" :class="r.decision === 'permit' ? 'st-verified' : 'st-failing'">{{ r.decision }}</span>
                <small v-if="r.reason" class="muted"> {{ r.reason }}</small></td>
              <td><code>{{ shortRev(r.model_revision) }}</code></td><td>{{ r.transport }}</td><td><code>{{ r.correlation_id }}</code></td>
            </tr>
          </tbody>
        </table>
        <p v-if="!audit.records.length" class="empty">No records match.</p>
      </template>
    </div>
  </section>
</template>

<style scoped>
.assurance { color: #e5edf9; }
.a-head { display: flex; justify-content: space-between; gap: 1.5rem; flex-wrap: wrap; align-items: flex-start; }
.a-head h2 { margin: 0 0 .4rem; font-size: 1.2rem; font-weight: 700; line-height: 1.3; }
.eyebrow { margin: 0 0 .3rem; color: #7dd3fc; font-size: .73rem; font-weight: 800; text-transform: uppercase; letter-spacing: .1em; }
.lede { margin: 0; max-width: 46rem; color: #b9c6dd; font-size: .92rem; line-height: 1.5; }
.a-revs { display: flex; gap: .8rem; align-items: center; flex-wrap: wrap; color: #a7b5d4; font-size: .85rem; }
code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .82em; color: #cfe4ff; background: #0d1630; border-radius: .25rem; padding: .05rem .3rem; }
.a-tabs { display: flex; gap: .3rem; margin: 1.2rem 0 1rem; border-bottom: 1px solid #29345a; }
.a-tabs button { border: 0; background: transparent; color: #a7b5d4; padding: .5rem .9rem; cursor: pointer; font-weight: 700; border-bottom: 2px solid transparent; }
.a-tabs button.active { color: #fff; border-bottom-color: #38bdf8; }
.a-body { display: grid; gap: 1.5rem; grid-template-columns: minmax(0, 1fr); }
.a-body.withDetail { grid-template-columns: minmax(0, 1.3fr) minmax(20rem, 1fr); align-items: start; }
.chips { list-style: none; margin: 0 0 1rem; padding: 0; display: flex; flex-wrap: wrap; gap: .5rem; }
.chip { display: inline-flex; gap: .35rem; align-items: center; border: 1px solid; border-radius: 999px; padding: .3rem .7rem; font-size: .82rem; background: transparent; }
button.chip { cursor: pointer; }
button.chip.on { box-shadow: 0 0 0 2px #38bdf8; }
.state { display: inline-flex; gap: .3rem; align-items: center; border: 1px solid; border-radius: .35rem; padding: .1rem .5rem; font-size: .8rem; font-weight: 700; white-space: nowrap; }
.st-verified { color: #4ade80; border-color: #1f6b3d; background: #0f2a1c; }
.st-satisfied_untested { color: #fbbf24; border-color: #7a5a10; background: #2a2208; }
.st-failing { color: #f87171; border-color: #7a2a2a; background: #2a0f0f; }
.st-stale { color: #fb923c; border-color: #7a4310; background: #2a1808; }
.st-unsatisfied { color: #94a3b8; border-color: #475569; background: #131b2e; }
.filters { display: flex; flex-wrap: wrap; gap: .8rem; align-items: end; margin-bottom: .6rem; }
.filters label { display: grid; gap: .25rem; color: #b9c7df; font-size: .8rem; }
.filters select, .filters input { border: 1px solid #3b4d7d; border-radius: .4rem; background: #091127; color: #edf5ff; padding: .4rem .55rem; min-width: 9rem; }
.ghost { border: 1px solid #3b4d7d; border-radius: .4rem; background: transparent; color: #9cc9ff; padding: .4rem .8rem; cursor: pointer; font-weight: 600; }
.ghost:disabled { opacity: .5; cursor: default; }
.go { border: 0; border-radius: .4rem; background: #38bdf8; color: #052235; padding: .4rem .9rem; font-weight: 800; cursor: pointer; }
.go:disabled { opacity: .6; cursor: wait; }
.count { margin: .4rem 0; color: #a7b5d4; font-size: .85rem; }
.count.ok { color: #4ade80; } .count.bad { color: #f87171; }
.req-table { width: 100%; border-collapse: collapse; font-size: .88rem; }
.req-table th { text-align: left; color: #7f8db2; font-size: .72rem; text-transform: uppercase; letter-spacing: .04em; padding: .4rem .6rem; border-bottom: 1px solid #29345a; }
.req-table td { padding: .6rem; border-bottom: 1px solid #1d2850; vertical-align: top; }
.req-table tr.selected td { background: #182246; }
.req-table tr.deny td { background: #2a0f0f55; }
.link { border: 0; background: transparent; color: #cfe4ff; cursor: pointer; text-align: left; padding: 0; font-size: .92rem; }
.link:hover { text-decoration: underline; }
.sub { color: #8d9bbd; font-size: .78rem; margin-top: .2rem; }
.sat { color: #a7b5d4; }
.gaps { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: .3rem; }
.gaps li { font-size: .72rem; border: 1px solid #3d5288; border-radius: .3rem; padding: .05rem .35rem; color: #b6c6e6; cursor: help; }
.badge { font-size: .68rem; font-weight: 700; text-transform: uppercase; letter-spacing: .03em; border: 1px solid #3d5288; border-radius: .3rem; padding: .02rem .35rem; color: #9cc9ff; margin-right: .3rem; }
.detail { background: #131d3c; border: 1px solid #2a3966; border-radius: .8rem; padding: 1rem 1.2rem; position: sticky; top: 1rem; max-height: calc(100vh - 2rem); overflow: auto; }
.detail h3 { margin: 0 0 .5rem; display: flex; gap: .6rem; align-items: center; flex-wrap: wrap; font-size: 1.3rem; font-weight: 800; line-height: 1.2; }
.detail h4 { margin: 1.1rem 0 .4rem; font-size: .8rem; color: #7dd3fc; text-transform: uppercase; letter-spacing: .05em; }
.detail h4 small { color: #8d9bbd; text-transform: none; letter-spacing: 0; font-weight: 400; }
.close { float: right; }
.statement { margin: 0 0 .6rem; padding: .6rem .8rem; border-left: 3px solid #38bdf8; background: #0d1630; border-radius: 0 .4rem .4rem 0; line-height: 1.45; }
.lints { margin: 0 0 .6rem; padding-left: 1.1rem; color: #fbbf24; font-size: .85rem; }
.facts { display: grid; grid-template-columns: 6rem 1fr; gap: .25rem .8rem; margin: 0; font-size: .85rem; }
.facts dt { color: #8d9bbd; } .facts dd { margin: 0; }
.thread { list-style: none; margin: 0; padding: 0; display: grid; gap: .5rem; font-size: .86rem; }
.thread li { padding-left: .7rem; border-left: 2px solid #2a3966; }
.impl { display: inline-block; margin-top: .2rem; word-break: break-all; }
.run { margin-top: .4rem; display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; }
.ok { color: #4ade80; } .bad { color: #f87171; } .stale { color: #fb923c; font-weight: 700; } .muted { color: #8d9bbd; }
.notice { border: 1px solid; border-radius: .5rem; padding: .6rem .8rem; margin: .6rem 0; font-size: .88rem; }
.notice.bad { color: #fecaca; border-color: #7a2a2a; background: #2a0f0f; }
.notice.ok { color: #bbf7d0; border-color: #1f6b3d; background: #0f2a1c; }
.empty { color: #8d9bbd; }
.legend { display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; margin-bottom: .8rem; }
.frame { overflow: auto; max-height: 75vh; background: #fff; border-radius: .5rem; border: 1px solid #2a3966; }
.diagram { display: block; max-width: none; background: #fff; padding: .5rem; box-sizing: border-box; }
.zoom { display: flex; gap: .4rem; align-items: center; margin: 0 0 .6rem; }
.zoom .on { background: #243465; color: #fff; border-color: #38bdf8; }
.zoom a { color: #9cc9ff; font-size: .85rem; margin-left: .5rem; }
@media (max-width: 70rem) { .a-body.withDetail { grid-template-columns: minmax(0, 1fr); } .detail { position: static; max-height: none; } }
</style>
