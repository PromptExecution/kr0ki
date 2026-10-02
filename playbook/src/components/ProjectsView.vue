<script setup>
// Local SysML v2 projects: edit files, Save (commit) with a message, see which requirements each change touched, the requirement
// graph, coverage gaps and a cost roll-up under scenarios. Storage is Quasar's LocalStorage (src/lib/projects.js).
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { LocalStorage } from 'quasar'
import CodeEditor from './CodeEditor.vue'
import { createProjectStore, fileKind, QuotaError } from '../lib/projects.js'
import { declaredRequirements, tracesOf } from '../lib/sysmlText.js'
import { attributionSummary, buildRequirementGraph, coverageGaps, duplicateIds, rollUpCosts, toD2 } from '../lib/requirementsGraph.js'
import { CODE_RE } from '../lib/sysmlText.js'
import { FRAMEWORKS, complianceCoverage, parseTag } from '../lib/compliance.js'
import { reqifToSysml } from '../lib/reqif.js'

const props = defineProps({
  rendererUrl: { type: String, default: '' },
  store: { type: Object, default: null }, // injected in tests
})
const store = props.store || createProjectStore({ storage: LocalStorage })

const projects = ref([])
const currentId = ref('')
const work = reactive({}) // the working tree: { path: text }
const baseline = ref({}) // files at HEAD
const extraTraces = ref([]) // links added by hand or by ReqIF import (diagram depicts requirement, ...)
const selectedPath = ref('')
const message = ref('')
const notice = ref(null) // { kind: 'ok'|'error', text }
const tab = ref('history')
const history = ref([])
const scenario = ref(null) // null = everything, else Set of requirement ids
const graphUrl = ref('')
const graphBusy = ref(false)
const newName = ref('')
const newFile = ref('')
const depictId = ref('')
const framework = ref('au-ai6')
const link = reactive({ requirement: '', kind: 'attribution', value: '', share: 1, basis: 'judgement' })
const CODEBOOK_PATH = 'attribution/codebook.json'
const usage = ref({ bytes: 0, budgetBytes: 1, fraction: 0 })

// Vue wraps state in proxies; storage engines (and structuredClone) want plain data.
const plain = (v) => JSON.parse(JSON.stringify(v))
const say = (kind, text) => { notice.value = { kind, text } }
const current = computed(() => projects.value.find((p) => p.id === currentId.value) || null)

function refresh() {
  projects.value = store.listProjects()
  usage.value = store.usage()
  if (currentId.value) history.value = store.log(currentId.value, { limit: 100 })
}

function open(id) {
  currentId.value = id
  const p = store.getProject(id)
  baseline.value = p?.head ? store.checkout(id, p.head) : {}
  for (const key of Object.keys(work)) delete work[key]
  const draft = store.loadDraft(id)
  Object.assign(work, draft ? draft.files : baseline.value)
  extraTraces.value = draft?.traces || []
  selectedPath.value = draft?.selected && draft.selected in work ? draft.selected : Object.keys(work)[0] || ''
  scenario.value = null
  graphUrl.value = ''
  message.value = ''
  refresh()
}

onMounted(() => {
  refresh()
  if (projects.value.length) open(projects.value[0].id)
})

// the unsaved draft is persisted as you work
let draftTimer = null
watch([() => ({ ...work }), extraTraces, selectedPath], () => {
  if (!currentId.value) return
  clearTimeout(draftTimer)
  draftTimer = setTimeout(() => { try { store.saveDraft(currentId.value, { files: plain(work), traces: plain(extraTraces.value), selected: selectedPath.value }) } catch { /* a full store must not break editing */ } }, 300)
}, { deep: true })

const paths = computed(() => Object.keys(work).sort())
const changes = computed(() => {
  const out = []
  for (const p of new Set([...Object.keys(work), ...Object.keys(baseline.value)])) {
    if (!(p in baseline.value)) out.push({ path: p, status: 'added' })
    else if (!(p in work)) out.push({ path: p, status: 'removed' })
    else if (work[p] !== baseline.value[p]) out.push({ path: p, status: 'modified' })
  }
  return out
})
const statusOf = (p) => changes.value.find((c) => c.path === p)?.status || ''

// everything derived from the working tree, live
const declared = computed(() => paths.value.filter((p) => fileKind(p) === 'sysml').flatMap((p) => declaredRequirements(work[p]).map((d) => ({ ...d, artifact: p }))))
const traces = computed(() => [...paths.value.filter((p) => fileKind(p) === 'sysml').flatMap((p) => tracesOf(p, work[p])), ...extraTraces.value])
const graph = computed(() => buildRequirementGraph(traces.value, declared.value))
const gaps = computed(() => coverageGaps(graph.value))
const duplicates = computed(() => duplicateIds(graph.value))
// Optional code book: attribution/codebook.json = { codes: [{ code, name, owner }] }. When present, unknown codes are flagged.
const codebook = computed(() => {
  if (!(CODEBOOK_PATH in work)) return null
  try { return new Set((JSON.parse(work[CODEBOOK_PATH]).codes || []).map((c) => c.code)) } catch { return null }
})
const attribution = computed(() => attributionSummary(graph.value, { include: scenario.value, codebook: codebook.value }))
const coverage = computed(() => complianceCoverage(graph.value, framework.value, { include: scenario.value }))
const hasLegacyCost = computed(() => graph.value.nodes.some((n) => n.cost !== null))
const manualLinks = computed(() => extraTraces.value.filter((t) => t.relation === 'attributed' || t.relation === 'tagged'))
const rollup = computed(() => rollUpCosts(graph.value, scenario.value))
const depictsHere = computed(() => extraTraces.value.filter((t) => t.relation === 'depicts' && t.artifact === selectedPath.value))

function createProject() {
  try {
    const p = store.createProject({ name: newName.value })
    newName.value = ''
    open(p.id)
    say('ok', `Created project "${p.name}". Add a file, then Save.`)
  } catch (e) { say('error', e.message) }
}

function deleteProject() {
  if (!current.value || !globalThis.confirm?.(`Delete project "${current.value.name}" and its whole history?`)) return
  store.deleteProject(currentId.value)
  currentId.value = ''
  for (const key of Object.keys(work)) delete work[key]
  refresh()
  if (projects.value.length) open(projects.value[0].id)
}

const TEMPLATES = {
  sysml: (name) => `package ${name.replace(/\.[^.]+$/, '').replace(/[^A-Za-z0-9_]/g, '_') || 'Model'} {\n  requirement <'REQ-1'> req_1 {\n    doc /* The system shall ... */\n    // Charge it to an accounting code (not an amount), for example:\n    // @CostAttribution { code = 'CC-4410/WBS-2.3'; share = 1; }\n    // Tag it against a compliance framework, for example:\n    // @ComplianceTag { tag = 'au-ai6:P5'; basis = 'judgement'; }\n  }\n}\n`,
  diagram: () => 'a -> b\n',
}
function addFile() {
  const path = newFile.value.trim()
  if (!path) return
  if (path in work) return say('error', `${path} already exists`)
  work[path] = (TEMPLATES[fileKind(path)] || TEMPLATES.diagram)(path.split('/').pop())
  selectedPath.value = path
  newFile.value = ''
}
function removeFile(path) {
  delete work[path]
  extraTraces.value = extraTraces.value.filter((t) => t.artifact !== path)
  if (selectedPath.value === path) selectedPath.value = Object.keys(work)[0] || ''
}

function addDepicts() {
  const id = depictId.value.trim()
  if (!id || !selectedPath.value) return
  if (!extraTraces.value.some((t) => t.requirement === id && t.relation === 'depicts' && t.artifact === selectedPath.value)) {
    extraTraces.value = [...extraTraces.value, { requirement: id, relation: 'depicts', target: null, artifact: selectedPath.value }]
  }
  depictId.value = ''
}
function addLink() {
  const id = link.requirement
  const value = link.value.trim()
  if (!id || !value) return say('error', 'Pick a requirement and enter a value.')
  if (link.kind === 'attribution') {
    if (!CODE_RE.test(value)) return say('error', `"${value}" is not a valid attribution code. Use letters, digits and - _ . within each part, parts separated by / (for example CC-4410/WBS-2.3).`)
    const share = Number(link.share)
    if (!(share > 0 && share <= 1)) return say('error', 'The share must be above 0 and at most 1.')
    extraTraces.value = [...extraTraces.value, { requirement: id, relation: 'attributed', target: value, artifact: '(project)', share }]
  } else {
    if (!parseTag(value)) return say('error', `"${value}" is not a tag. Use framework:control, for example au-ai6:P5 or iso42001:A.6.2.6.`)
    extraTraces.value = [...extraTraces.value, { requirement: id, relation: 'tagged', target: value, artifact: '(project)', basis: link.basis }]
  }
  link.value = ''
}
const removeLink = (t) => { extraTraces.value = extraTraces.value.filter((x) => x !== t) }
function createCodebook() {
  if (CODEBOOK_PATH in work) { selectedPath.value = CODEBOOK_PATH; return }
  work[CODEBOOK_PATH] = JSON.stringify({ codes: [{ code: 'CC-0000', name: 'Replace with your first accounting code', owner: '' }] }, null, 2) + '\n'
  selectedPath.value = CODEBOOK_PATH
  say('ok', 'Created attribution/codebook.json. List your valid codes there; unknown codes are then flagged.')
}
const removeDepicts = (t) => { extraTraces.value = extraTraces.value.filter((x) => x !== t) }

function save() {
  if (!currentId.value) return say('error', 'Create or open a project first.')
  try {
    const c = store.commit(currentId.value, { files: plain(work), message: message.value, traces: plain(extraTraces.value) })
    if (!c) return say('ok', 'Nothing changed since the last save.')
    store.saveDraft(currentId.value, null)
    baseline.value = store.checkout(currentId.value, c.id)
    message.value = ''
    refresh()
    const n = c.impact.requirements.length
    say('ok', `Saved ${c.id.slice(0, 8)}: ${c.changes.length} file${c.changes.length === 1 ? '' : 's'} changed, ${n} requirement${n === 1 ? '' : 's'} affected${n ? ` (${c.impact.requirements.join(', ')})` : ''}.`)
  } catch (e) { say('error', e instanceof QuotaError ? e.message : `Could not save: ${e.message}`) }
}

function restore(commit) {
  for (const key of Object.keys(work)) delete work[key]
  Object.assign(work, store.checkout(currentId.value, commit.id))
  selectedPath.value = Object.keys(work)[0] || ''
  say('ok', `Loaded ${commit.id.slice(0, 8)} into the editor. Save to make it the new latest version.`)
}

const timeline = ref({ id: '', rows: [] })
function showHistoryOf(id) {
  timeline.value = { id, rows: store.requirementHistory(currentId.value, id) }
  tab.value = 'history'
}

function toggleInScenario(id, on) {
  const all = new Set(graph.value.nodes.map((n) => n.id))
  const s = new Set(scenario.value ?? all) // a COPY: `all` is compared against below
  if (on) s.add(id); else s.delete(id)
  scenario.value = s.size === all.size ? null : s
}
const includedIn = (id) => scenario.value === null || scenario.value.has(id)

async function drawGraph() {
  if (!graph.value.nodes.length) return say('error', 'No requirements to draw yet.')
  graphBusy.value = true
  try {
    const base = (props.rendererUrl || window.location.origin).replace(/\/$/, '')
    const res = await fetch(`${base}/render/d2?output=svg`, { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: toD2(graph.value, rollup.value) })
    if (!res.ok) throw new Error(`renderer returned ${res.status}`)
    graphUrl.value = URL.createObjectURL(await res.blob())
  } catch (e) { say('error', `Could not draw the graph: ${e.message}`) } finally { graphBusy.value = false }
}

// Ask the real SysML v2 parser (the MCP sidecar behind /sysml/validate) rather than the browser's heuristic scanner.
const validation = ref(null)
const validating = ref(false)
async function validateSysml() {
  const path = selectedPath.value
  if (!path || fileKind(path) !== 'sysml') return
  validating.value = true
  try {
    const base = (props.rendererUrl || window.location.origin).replace(/\/$/, '')
    const res = await fetch(`${base}/sysml/validate`, { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: work[path] })
    const body = await res.json()
    if (!res.ok) throw new Error(body.message || `server returned ${res.status}`)
    validation.value = { path, syntax: body.syntaxErrors || [], semantic: body.semanticIssues || [] }
  } catch (e) { validation.value = null; say('error', `Could not validate: ${e.message}`) } finally { validating.value = false }
}
const issueLine = (i) => `${i.line != null ? `line ${i.line}: ` : ''}${i.message}`

async function importReqif(event) {
  const file = event.target.files?.[0]
  event.target.value = ''
  if (!file || !currentId.value) return
  if (!/\.reqif$/i.test(file.name)) return say('error', 'Only plain .reqif files can be stored in a local project (.reqifz bundles are binary).')
  try {
    const base = (props.rendererUrl || window.location.origin).replace(/\/$/, '')
    const res = await fetch(`${base}/requirements/import`, { method: 'POST', body: await file.arrayBuffer() })
    const body = await res.json()
    if (!res.ok) throw new Error(body.message || `import failed (${res.status})`)
    const stem = file.name.replace(/\.reqif$/i, '').replace(/[^A-Za-z0-9_.-]/g, '_')
    const reqifPath = `requirements/${stem}.reqif`
    const sysmlPath = `requirements/${stem}.sysml`
    const out = reqifToSysml(body, { sysmlPath, reqifPath, packageName: stem })
    work[reqifPath] = await file.text()
    work[sysmlPath] = out.sysml
    extraTraces.value = [...extraTraces.value.filter((t) => t.artifact !== reqifPath), ...out.traces]
    selectedPath.value = sysmlPath
    say('ok', `Imported ${out.count} requirement${out.count === 1 ? '' : 's'} from ${file.name}. Review, then Save.`)
  } catch (e) { say('error', `ReqIF import failed: ${e.message}`) }
}

function exportBundle() {
  const blob = new Blob([JSON.stringify(store.exportProject(currentId.value), null, 1)], { type: 'application/json' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `${(current.value?.name || 'project').replace(/[^A-Za-z0-9_-]/g, '_')}.kr0ki-project.json`
  a.click()
}
async function importBundle(event) {
  const file = event.target.files?.[0]
  event.target.value = ''
  if (!file) return
  try {
    const p = store.importProject(JSON.parse(await file.text()))
    refresh(); open(p.id)
    say('ok', `Imported project "${p.name}" with its history.`)
  } catch (e) { say('error', `Import failed: ${e.message}`) }
}

const pct = computed(() => Math.round(usage.value.fraction * 100))
const fmtTime = (t) => new Date(t).toLocaleString()
</script>

<template>
  <section class="projects" data-testid="projects-view">
    <header class="projects__bar">
      <h2>Projects</h2>
      <select v-if="projects.length" :value="currentId" aria-label="Project" data-testid="project-select" @change="open($event.target.value)">
        <option v-for="p in projects" :key="p.id" :value="p.id">{{ p.name }}</option>
      </select>
      <form class="projects__new" @submit.prevent="createProject">
        <input v-model="newName" placeholder="New project name" aria-label="New project name" data-testid="new-project-name" />
        <button type="submit" data-testid="create-project">Create</button>
      </form>
      <span class="projects__usage" :title="`${usage.bytes} bytes of a ${usage.budgetBytes}-byte budget`" :data-warn="pct >= 80 || undefined" data-testid="usage">storage {{ pct }}%</span>
      <button v-if="current" type="button" class="secondary" @click="exportBundle">Export</button>
      <label class="secondary file-btn">Import project<input type="file" accept=".json,application/json" @change="importBundle" /></label>
      <button v-if="current" type="button" class="secondary danger" @click="deleteProject">Delete</button>
    </header>
    <p v-if="notice" class="projects__notice" :data-kind="notice.kind" role="status" data-testid="notice">{{ notice.text }}</p>

    <p v-if="!current" class="projects__empty" data-testid="projects-empty">
      A project keeps your SysML v2 text, diagrams and requirements together, with every save recorded as a commit that shows which requirements it affected.
      Create one to begin. Projects are stored in this browser (export them to keep a copy).
    </p>

    <div v-else class="projects__main">
      <aside class="projects__files">
        <h3>Files <span class="muted">· {{ changes.length }} unsaved</span></h3>
        <ul>
          <li v-for="p in paths" :key="p" :class="{ active: p === selectedPath }">
            <button type="button" class="file" :data-testid="`file-${p}`" @click="selectedPath = p">
              <span class="file-kind">{{ fileKind(p) }}</span>{{ p }}<span v-if="statusOf(p)" class="file-status" :data-status="statusOf(p)">{{ statusOf(p) }}</span>
            </button>
            <button type="button" class="x" :aria-label="`Remove ${p}`" @click="removeFile(p)">×</button>
          </li>
        </ul>
        <form class="projects__addfile" @submit.prevent="addFile">
          <input v-model="newFile" placeholder="model/vehicle.sysml" aria-label="New file path" data-testid="new-file-path" />
          <button type="submit" data-testid="add-file">Add</button>
        </form>
        <label class="secondary file-btn">Import ReqIF<input type="file" accept=".reqif,.xml" data-testid="reqif-input" @change="importReqif" /></label>
      </aside>

      <div class="projects__work">
        <template v-if="selectedPath">
          <p class="projects__path">{{ selectedPath }}</p>
          <CodeEditor v-model="work[selectedPath]" :format="''" data-testid="project-editor" />
          <div v-if="fileKind(selectedPath) === 'sysml'" class="projects__validate">
            <button type="button" class="secondary" :disabled="validating" data-testid="validate-sysml" @click="validateSysml">{{ validating ? 'Validating…' : 'Validate with SysML v2 parser' }}</button>
            <template v-if="validation && validation.path === selectedPath">
              <span v-if="!validation.syntax.length" class="muted" data-testid="validation-ok">No syntax errors<template v-if="validation.semantic.length"> · {{ validation.semantic.length }} semantic note{{ validation.semantic.length === 1 ? '' : 's' }}</template>.</span>
              <ul v-if="validation.syntax.length || validation.semantic.length" class="projects__issues" data-testid="validation-issues">
                <li v-for="(i, n) in validation.syntax" :key="`s${n}`" class="issue-error">{{ issueLine(i) }}</li>
                <li v-for="(i, n) in validation.semantic.slice(0, 20)" :key="`m${n}`" class="issue-note">{{ issueLine(i) }}</li>
              </ul>
            </template>
          </div>
          <div v-if="fileKind(selectedPath) === 'diagram'" class="projects__depicts">
            <span>This diagram depicts requirement:</span>
            <span v-for="t in depictsHere" :key="t.requirement" class="chip">{{ t.requirement }}<button type="button" :aria-label="`Unlink ${t.requirement}`" @click="removeDepicts(t)">×</button></span>
            <input v-model="depictId" placeholder="REQ-1" aria-label="Requirement id" data-testid="depicts-id" @keydown.enter.prevent="addDepicts" />
            <button type="button" class="secondary" data-testid="add-depicts" @click="addDepicts">Link</button>
          </div>
        </template>
        <p v-else class="muted">Add a file to start (try <code>model/vehicle.sysml</code>).</p>
        <form class="projects__commit" @submit.prevent="save">
          <input v-model="message" placeholder="What changed and why? (commit message)" aria-label="Commit message" data-testid="commit-message" />
          <button type="submit" data-testid="save">Save (commit)</button>
        </form>
      </div>
    </div>

    <div v-if="current" class="projects__tabs">
      <nav role="tablist">
        <button v-for="t in ['history', 'requirements', 'attribution', 'compliance', 'graph']" :key="t" role="tab" :aria-selected="tab === t" :class="{ active: tab === t }" :data-testid="`tab-${t}`" @click="tab = t">
          {{ t === 'history' ? `History (${history.length})` : t === 'requirements' ? `Requirements (${graph.nodes.length})` : t === 'attribution' ? 'Cost attribution' : t === 'compliance' ? 'Compliance' : 'Graph' }}
        </button>
      </nav>

      <div v-show="tab === 'history'" class="projects__panel" data-testid="panel-history">
        <template v-if="timeline.id">
          <p class="muted">History of <strong>{{ timeline.id }}</strong> <button type="button" class="link" @click="timeline = { id: '', rows: [] }">show all commits</button></p>
          <p v-if="!timeline.rows.length" class="muted">No saved commit has touched this requirement yet.</p>
          <ol v-else><li v-for="r in timeline.rows" :key="r.commit"><strong>{{ r.message }}</strong> <span class="muted">{{ fmtTime(r.time) }} · {{ r.changes.map((c) => `${c.status} ${c.path}`).join(', ') }}</span></li></ol>
        </template>
        <template v-else>
          <p v-if="!history.length" class="muted">Nothing saved yet.</p>
          <ol v-else class="commits">
            <li v-for="c in history" :key="c.id" :data-testid="`commit-${c.id.slice(0, 8)}`">
              <div><strong>{{ c.message }}</strong> <code>{{ c.id.slice(0, 8) }}</code> <span class="muted">{{ fmtTime(c.time) }}</span></div>
              <div class="muted">{{ c.changes.map((x) => `${x.status} ${x.path}`).join(' · ') }}</div>
              <div v-if="c.impact.requirements.length">affects <button v-for="r in c.impact.requirements" :key="r" type="button" class="chip link" @click="showHistoryOf(r)">{{ r }}</button></div>
              <button type="button" class="link" @click="restore(c)">Load this version</button>
            </li>
          </ol>
        </template>
      </div>

      <div v-show="tab === 'requirements'" class="projects__panel" data-testid="panel-requirements">
        <p v-if="!graph.nodes.length" class="muted">No requirements found. Declare one in a .sysml file: <code>requirement &lt;'REQ-1'&gt; name { doc /* text */ }</code></p>
        <table v-else>
          <thead><tr><th>Include</th><th>Id</th><th>Text</th><th>Charged to (code · share)</th><th>Compliance tags</th><th>Satisfied by</th><th>Verified</th><th></th></tr></thead>
          <tbody>
            <tr v-for="n in graph.nodes" :key="n.id" :data-testid="`req-${n.id}`">
              <td><input type="checkbox" :checked="includedIn(n.id)" :aria-label="`Include ${n.id}`" @change="toggleInScenario(n.id, $event.target.checked)" /></td>
              <td><code>{{ n.id }}</code></td>
              <td>{{ n.title }}</td>
              <td :class="{ gap: !n.attributions.length }" data-col="attribution">
                <span v-for="a in n.attributions" :key="a.code + a.share" class="chip">{{ a.code }} · {{ Math.round(a.share * 100) }}%</span>
                <template v-if="!n.attributions.length">not charged</template>
              </td>
              <td data-col="tags"><span v-for="t in n.tags" :key="t.tag" class="chip" :title="t.basis">{{ t.tag }}</span></td>
              <td :class="{ gap: !n.satisfiedBy.length }">{{ n.satisfiedBy.join(', ') || 'nothing yet' }}</td>
              <td :class="{ gap: !n.verifiedBy.length }">{{ n.verifiedBy.length ? 'yes' : 'no' }}</td>
              <td><button type="button" class="link" @click="showHistoryOf(n.id)">history</button></td>
            </tr>
          </tbody>
        </table>
        <form v-if="graph.nodes.length" class="projects__link" data-testid="add-link" @submit.prevent="addLink">
          <strong>Add to a requirement:</strong>
          <select v-model="link.requirement" aria-label="Requirement" data-testid="link-requirement"><option value="" disabled>requirement…</option><option v-for="n in graph.nodes" :key="n.id" :value="n.id">{{ n.id }}</option></select>
          <select v-model="link.kind" aria-label="Kind" data-testid="link-kind"><option value="attribution">attribution code</option><option value="tag">compliance tag</option></select>
          <input v-model="link.value" :placeholder="link.kind === 'attribution' ? 'CC-4410/WBS-2.3' : 'au-ai6:P5'" aria-label="Value" data-testid="link-value" />
          <input v-if="link.kind === 'attribution'" v-model.number="link.share" type="number" min="0" max="1" step="any" aria-label="Share" data-testid="link-share" />
          <select v-else v-model="link.basis" aria-label="Basis" data-testid="link-basis"><option value="judgement">judgement</option><option value="official">official crosswalk</option></select>
          <button type="submit" data-testid="link-add">Add</button>
        </form>
        <p v-if="manualLinks.length" class="muted">Added here (saved with the project):
          <span v-for="t in manualLinks" :key="t.requirement + t.relation + t.target + t.share" class="chip">{{ t.requirement }} → {{ t.target }}<template v-if="t.relation === 'attributed'"> · {{ Math.round(t.share * 100) }}%</template><button type="button" :aria-label="`Remove ${t.target} from ${t.requirement}`" @click="removeLink(t)">×</button></span>
        </p>
        <p v-if="hasLegacyCost" class="projects__total muted" data-testid="scenario-total">
          Legacy numeric weight (from <code>attribute cost</code>, kept for old projects; not an accounting amount): <strong>{{ rollup.total }}</strong>
          <button v-if="scenario" type="button" class="link" @click="scenario = null">include all</button>
        </p>
        <p v-if="scenario" class="muted">A scenario is active: {{ scenario.size }} requirement{{ scenario.size === 1 ? '' : 's' }} included. <button type="button" class="link" @click="scenario = null">include all</button></p>
        <p v-if="duplicates.length" class="projects__warn" data-testid="duplicates">
          Same id in more than one file: <span v-for="d in duplicates" :key="d.id"><code>{{ d.id }}</code> ({{ d.files.join(', ') }}) </span>. They are merged into one requirement here; ids should be unique.
        </p>
        <p v-if="gaps.length" class="muted" data-testid="gaps">{{ gaps.length }} requirement{{ gaps.length === 1 ? '' : 's' }} not yet fully covered (no <code>satisfy</code> and/or <code>verify</code> link).</p>
      </div>

      <div v-show="tab === 'attribution'" class="projects__panel" data-testid="panel-attribution">
        <p class="muted">Requirements are <strong>charged to accounting-style codes</strong> (cost centre / work breakdown / activity), not given a dollar amount. Shares are fractions of a requirement. Budget estimates per code are a later layer.</p>
        <p v-if="!graph.nodes.length" class="muted">No requirements yet.</p>
        <template v-else>
          <table v-if="attribution.byPrefix.length" data-testid="attribution-table">
            <thead><tr><th>Code (and every parent)</th><th>Requirements</th><th>Share</th></tr></thead>
            <tbody><tr v-for="r in attribution.byPrefix" :key="r.prefix"><td><code>{{ r.prefix }}</code></td><td>{{ r.requirements.join(', ') }}</td><td>{{ r.share }}</td></tr></tbody>
          </table>
          <p v-else class="muted">No codes assigned yet. Add one in the Requirements tab or with <code>@CostAttribution</code> in SysML.</p>
          <ul class="projects__issues" data-testid="attribution-issues">
            <li v-if="attribution.unattributed.length" class="gap">Not charged to any code: {{ attribution.unattributed.join(', ') }}</li>
            <li v-for="o in attribution.overAllocated" :key="'o' + o.id" class="gap">{{ o.id }}: shares add up to {{ o.total }} (over 1)</li>
            <li v-for="o in attribution.partlyAttributed" :key="'p' + o.id">{{ o.id }}: only {{ o.total }} of the requirement is charged to a code</li>
            <li v-for="c in attribution.invalidCodes" :key="'i' + c" class="gap">Not a valid code: <code>{{ c }}</code></li>
            <li v-for="u in attribution.unknownCodes" :key="'u' + u.code" class="gap">Not in the code book: <code>{{ u.code }}</code> ({{ u.requirements.join(', ') }})</li>
          </ul>
          <p class="muted">Code book: <template v-if="codebook">{{ codebook.size }} code{{ codebook.size === 1 ? '' : 's' }} listed.</template><template v-else>none yet (codes are not checked).</template>
            <button type="button" class="link" data-testid="codebook" @click="createCodebook">{{ codebook ? 'Open the code book' : 'Create a code book' }}</button></p>
        </template>
      </div>

      <div v-show="tab === 'compliance'" class="projects__panel" data-testid="panel-compliance">
        <label>Framework
          <select v-model="framework" data-testid="framework-select"><option v-for="(fw, slug) in FRAMEWORKS" :key="slug" :value="slug">{{ slug }}: {{ fw.name }}</option></select>
        </label>
        <p class="muted">
          Source: <a :href="FRAMEWORKS[framework].url" target="_blank" rel="noopener">{{ FRAMEWORKS[framework].url }}</a> · control ids are
          <strong>{{ FRAMEWORKS[framework].confidence }}</strong>{{ FRAMEWORKS[framework].confidence === 'recalled' ? ' (from the published text as remembered, not re-fetched: verify before relying on an id)' : '' }}.
          <span v-if="FRAMEWORKS[framework].note"> {{ FRAMEWORKS[framework].note }}</span> Working aid, not legal advice.
        </p>
        <table data-testid="coverage-table">
          <thead><tr><th>Control</th><th>Name</th><th>Requirements</th><th>Basis</th></tr></thead>
          <tbody>
            <tr v-for="r in coverage.rows" :key="r.control" :data-testid="`ctl-${r.control}`">
              <td><code>{{ r.control }}</code></td><td>{{ r.name }}</td>
              <td :class="{ gap: !r.requirements.length }">{{ r.requirements.join(', ') || 'no requirement yet' }}</td>
              <td class="muted">{{ r.requirements.length ? `${r.official} official · ${r.judgement} judgement` : '' }}</td>
            </tr>
          </tbody>
        </table>
        <p class="muted" data-testid="coverage-summary">{{ coverage.rows.length - coverage.gaps.length }} of {{ coverage.rows.length }} controls have at least one requirement.</p>
        <ul class="projects__issues">
          <li v-if="coverage.untagged.length">No compliance tag at all: {{ coverage.untagged.join(', ') }}</li>
          <li v-for="m in coverage.malformed" :key="'m' + m.requirement + m.tag" class="gap">{{ m.requirement }}: <code>{{ m.tag }}</code> is not a tag (use framework:control)</li>
          <li v-for="u in coverage.unknown" :key="'k' + u.requirement + u.tag" class="gap">{{ u.requirement }}: <code>{{ u.tag }}</code> is not a control in this catalogue</li>
        </ul>
      </div>

      <div v-show="tab === 'graph'" class="projects__panel" data-testid="panel-graph">
        <button type="button" :disabled="graphBusy" data-testid="draw-graph" @click="drawGraph">{{ graphBusy ? 'Drawing…' : 'Draw the requirement graph' }}</button>
        <span class="muted"> Edges point from a derived requirement to its parent; red outline = coverage gap; faded = excluded from the scenario.</span>
        <img v-if="graphUrl" :src="graphUrl" alt="Requirement graph" class="projects__graph" data-testid="graph-image" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.projects { display: flex; flex-direction: column; gap: .75rem; }
.projects__bar { display: flex; flex-wrap: wrap; align-items: center; gap: .6rem; }
.projects__bar h2 { margin: 0 .5rem 0 0; font-size: 1.1rem; }
.projects__new { display: flex; gap: .3rem; }
.projects input[type="text"], .projects input:not([type]), .projects select { background: #091127; color: #edf5ff; border: 1px solid #3b4d7d; border-radius: .4rem; padding: .35rem .5rem; }
.projects button { border: 0; border-radius: .4rem; padding: .35rem .7rem; font-weight: 700; font-size: .8rem; cursor: pointer; background: #38bdf8; color: #052235; }
.projects button.secondary, .projects .file-btn.secondary { background: transparent; color: #9cc9ff; border: 1px solid #3b4d7d; }
.projects button.danger { color: #fca5a5; border-color: #7f1d1d; }
.projects button.link { background: none; color: #7dd3fc; padding: 0; font-weight: 600; text-decoration: underline; }
.projects button:disabled { opacity: .55; cursor: wait; }
.file-btn { display: inline-flex; align-items: center; padding: .35rem .7rem; border-radius: .4rem; font-weight: 700; font-size: .8rem; cursor: pointer; }
.file-btn input { display: none; }
.projects__usage { font-size: .72rem; color: #6f7fa3; margin-left: auto; }
.projects__usage[data-warn] { color: #fbbf24; font-weight: 700; }
.projects__notice { margin: 0; padding: .45rem .7rem; border-radius: .5rem; font-size: .85rem; background: #12301f; border: 1px solid #1f6f43; color: #bbf7d0; }
.projects__notice[data-kind="error"] { background: #3b1219; border-color: #7f1d1d; color: #fecaca; }
.projects__empty, .muted { color: #8d9bbd; font-size: .85rem; }
.projects__main { display: grid; grid-template-columns: 15rem minmax(0, 1fr); gap: 1rem; align-items: start; }
.projects__files h3 { margin: 0 0 .4rem; font-size: .9rem; }
.projects__files ul { list-style: none; margin: 0 0 .5rem; padding: 0; display: grid; gap: 2px; }
.projects__files li { display: flex; align-items: center; border-radius: .35rem; }
.projects__files li.active { background: #243465; }
.file { flex: 1; min-width: 0; text-align: left; background: transparent !important; color: #ced8ee !important; font-weight: 500 !important; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-kind { color: #6f7fa3; font-size: .65rem; margin-right: .35rem; text-transform: uppercase; }
.file-status { margin-left: .4rem; font-size: .62rem; color: #fbbf24; }
.x { background: transparent !important; color: #8d9bbd !important; padding: 0 .4rem !important; }
.projects__addfile, .projects__commit { display: flex; gap: .4rem; }
.projects__addfile input, .projects__commit input { flex: 1; min-width: 0; }
.projects__path { margin: 0 0 .3rem; font-family: ui-monospace, monospace; font-size: .8rem; color: #9cc9ff; }
.projects__work { min-width: 0; display: flex; flex-direction: column; gap: .5rem; }
.projects__depicts { display: flex; flex-wrap: wrap; align-items: center; gap: .4rem; font-size: .8rem; color: #cdd8ee; }
.chip { display: inline-flex; align-items: center; gap: .2rem; padding: .05rem .5rem; border-radius: 999px; background: #243465; color: #bae6fd; font-size: .75rem; }
.chip button { background: none; color: inherit; padding: 0 .2rem; }
button.chip.link { text-decoration: none; margin-right: .25rem; }
.projects__tabs nav { display: flex; gap: .25rem; border-bottom: 1px solid #2a3966; }
.projects__tabs nav button { background: transparent; color: #a7b5d4; border-radius: .4rem .4rem 0 0; }
.projects__tabs nav button.active { background: #243465; color: #fff; }
.projects__panel { padding: .75rem .25rem; font-size: .85rem; color: #cdd8ee; }
.commits { list-style: none; padding: 0; margin: 0; display: grid; gap: .6rem; }
.commits li { padding: .5rem .7rem; background: #101a36; border: 1px solid #2a3966; border-radius: .5rem; display: grid; gap: .2rem; }
table { border-collapse: collapse; width: 100%; font-size: .8rem; }
th, td { text-align: left; padding: .3rem .5rem; border-bottom: 1px solid #1f2b52; }
th { color: #8d9bbd; font-weight: 700; font-size: .7rem; text-transform: uppercase; }
td.gap { color: #fca5a5; }
.projects__total { margin: .6rem 0 .2rem; }
.projects__link { display: flex; flex-wrap: wrap; align-items: center; gap: .4rem; margin: .6rem 0; font-size: .8rem; }
.projects__link input[type="number"] { width: 5rem; }
.projects__issues { margin: .5rem 0; padding-left: 1.1rem; font-size: .82rem; }
.projects__issues .gap { color: #fca5a5; }
.projects__warn { margin: .5rem 0; padding: .4rem .6rem; border-radius: .4rem; background: #3b2a0a; border: 1px solid #92400e; color: #fde68a; font-size: .8rem; }
.projects__graph { display: block; max-width: 100%; margin-top: .6rem; background: #fff; border-radius: .4rem; }
@media (max-width: 760px) { .projects__main { grid-template-columns: 1fr; } }
.projects__validate { margin: 0.5rem 0; display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; }
.projects__issues { margin: 0; padding-left: 1.2rem; font-size: 0.85rem; flex-basis: 100%; }
.issue-error { color: #c0392b; }
.issue-note { opacity: 0.8; }
</style>
