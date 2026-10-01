<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import RendererPanel from './components/RendererPanel.vue'
import Gallery from './components/Gallery.vue'
import StoryB00k from './components/StoryB00k.vue'
import Setup from './components/Setup.vue'
import CatalogTree from './components/CatalogTree.vue'
import PlannerView from './components/PlannerView.vue'
import { connectUiBridge, plannerSessionId } from './lib/uiBridge.js'
import { loadLspUrls, saveLspUrls } from './lib/lsp.js'

// Version from Cargo.toml (injected at build time by Vite)
const appVersion = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '0.0.0'

const examples = ref([])
const selectedFormat = ref('d2')
const selectedId = ref('')
const loadError = ref('')
// The gallery is the home page; the planner, editor, agent and setup are one click away in the left menu.
const viewMode = ref('gallery')
// The planner chat stays mounted once opened, so its conversation survives switching tabs.
const plannerMounted = ref(false)
watch(viewMode, (v) => { if (v === 'planner') plannerMounted.value = true })
// Source handed over from StoryB00k's EDIT button (agent-rendered diagram).
const editedSource = ref('')
const editedRoute = ref(null)
let editedSourcePending = false

// Setup settings (persisted to localStorage)
const rendererUrl = ref(
  localStorage.getItem('kr0ki:rendererUrl') ||
    (typeof window !== 'undefined' && window.location.hostname
      ? `${window.location.protocol}//${window.location.hostname}:8787`
      : 'http://127.0.0.1:8787')
)
// Optional language servers per diagram language (Setup -> Language servers), persisted.
const lspUrls = ref(loadLspUrls())
function updateLspUrls(urls) {
  lspUrls.value = saveLspUrls(urls)
}
const outputFormat = ref(localStorage.getItem('kr0ki:outputFormat') || 'svg')
const llmUrl = ref(localStorage.getItem('kr0ki:llmUrl') || `http://${window.location.hostname}:8002/v1`)
const llmKey = ref(localStorage.getItem('kr0ki:llmKey') || '')
const llmModel = ref(localStorage.getItem('kr0ki:llmModel') || 'gpt-4o')

function updateRendererUrl(url) {
  rendererUrl.value = url
  localStorage.setItem('kr0ki:rendererUrl', url)
}

function updateOutputFormat(format) {
  outputFormat.value = format
  localStorage.setItem('kr0ki:outputFormat', format)
}

function updateLlmUrl(url) {
  llmUrl.value = url
  localStorage.setItem('kr0ki:llmUrl', url)
}

function updateLlmKey(key) {
  llmKey.value = key
  localStorage.setItem('kr0ki:llmKey', key)
}

function updateLlmModel(model) {
  llmModel.value = model
  localStorage.setItem('kr0ki:llmModel', model)
}

const agentUrl = ref(
  localStorage.getItem('kr0ki:agentUrl') ||
    (typeof window !== 'undefined' && window.location.hostname
      ? `${window.location.protocol}//${window.location.hostname}:8789`
      : 'http://127.0.0.1:8789')
)

function updateAgentUrl(url) {
  agentUrl.value = url
  localStorage.setItem('kr0ki:agentUrl', url)
}
const agentPrefill = ref('')
const agentTypeId = ref('')
// ---- Gallery state, shared by the user and the planning agent -----------------------------------------------
// The planner (an LLM tool call, or any MCP client) arrives as UI commands over SSE; the user's own clicks go
// through the same refs, so both always see one consistent gallery.
const plannerSession = plannerSessionId()
const uiStatus = ref('closed')
const catalog = ref(null)
async function loadCatalog() {
  try {
    const base = (rendererUrl.value || window.location.origin).trim().replace(/\/$/, '')
    const res = await fetch(`${base}/api/catalog`)
    if (res.ok) catalog.value = await res.json()
  } catch (err) {
    console.warn('[playbook] catalog unavailable:', err?.message)
  }
}
watch(rendererUrl, loadCatalog)

const gallery = reactive({
  useCase: 'All',
  selected: new URL(window.location.href).searchParams.get('type') || '',
  suggested: [],
  note: '',
  // The planner's own best-fit pick (only ever set by the planner), kept apart from `selected`, which the user
  // also changes by clicking a card or a tree entry.
  pick: '',
})

function applyUiCommand(cmd) {
  switch (cmd.type) {
    case 'open_view':
      viewMode.value = cmd.view === 'agent' ? 'storyb00k' : cmd.view === 'planner' ? 'planner' : cmd.view
      break
    case 'filter_gallery':
      if (viewMode.value !== 'planner') viewMode.value = 'gallery'
      gallery.useCase = cmd.useCase
      break
    case 'suggest':
      // On the planner page the picks panel shows them; elsewhere bring the user to the gallery.
      if (viewMode.value !== 'planner') viewMode.value = 'gallery'
      gallery.suggested = cmd.typeIds
      gallery.note = cmd.note
      break
    case 'select_type':
      if (viewMode.value !== 'planner') viewMode.value = 'gallery'
      // Re-selecting the same type must still re-focus its card.
      gallery.pick = cmd.typeId
      gallery.selected = ''
      queueMicrotask(() => { gallery.selected = cmd.typeId })
      break
  }
}

let uiBridge = null
function connectBridge() {
  uiBridge?.close()
  uiBridge = connectUiBridge({
    baseUrl: rendererUrl.value,
    sessionId: plannerSession,
    onCommand: applyUiCommand,
    onStatus: (status) => { uiStatus.value = status },
  })
}
watch(rendererUrl, connectBridge)
onBeforeUnmount(() => uiBridge?.close())

// Editor → Agent handoff: diagram source with detected type
const editorHandoff = ref(null)

function agentHandoff({ prompt, typeId }) {
  agentPrefill.value = prompt
  agentTypeId.value = typeId
  editorHandoff.value = null
  viewMode.value = 'storyb00k'
}

function editorToAgentHandoff({ source, format, detectedType, output, imageData, title }) {
  editorHandoff.value = {
    source,
    format,
    detectedType,
    output,
    imageData,
    title,
  }
  // Simple prompt - the source will be sent separately
  agentPrefill.value = `Review this ${detectedType !== 'unknown' ? detectedType : format} diagram and suggest improvements.`
  agentTypeId.value = detectedType !== 'unknown' ? detectedType : ''
  viewMode.value = 'storyb00k'
}

// `api/examples.json` is deliberately relative: it resolves beneath
// /playbook/ in the live service and beneath /kr0ki/playbook/ on Pages.
const catalogUrl = new URL('api/examples.json', window.location.href)

const formatExamples = computed(() =>
  examples.value.filter((example) => example.format === selectedFormat.value),
)
const selectedExample = computed(() =>
  examples.value.find((example) => example.id === selectedId.value) || formatExamples.value[0],
)
const formats = computed(() => [...new Set(examples.value.map((example) => example.format))])

function selectFormat(format) {
  selectedFormat.value = format
  selectedId.value = examples.value.find((example) => example.format === format)?.id || ''
}

function onTreeSelect({ exampleId, typeId }) {
  const example = examples.value.find((e) => e.id === exampleId)
  if (!example) return
  if (typeId) gallery.selected = typeId
  openInEditor(example) // the editor renders it automatically
}

function showInGallery(typeId) {
  gallery.useCase = 'All'
  gallery.selected = typeId
  viewMode.value = 'gallery'
}

function openInEditor(example) {
  viewMode.value = 'editor'
  selectedFormat.value = example.format
  selectedId.value = example.id
}

// EDIT from a StoryB00k render panel: preload the agent-produced source into
// the editor. Host selection is format+route aware: a k8s-topology source must
// land on the k8s-topology example (its /render/k8s-topology route), not the
// d2 example — otherwise the editor POSTs YAML to /render/d2 and Kroki 400s.
function editInEditor({ source, format, route }) {
  const formatId = format || 'd2'
  const host = (formatId !== 'd2' && examples.value.find((example) => example.format === formatId))
    || examples.value.find((example) => example.format === 'd2')
  if (!host) return
  viewMode.value = 'editor'
  selectedFormat.value = host.format
  selectedId.value = host.id
  editedSource.value = source
  // Carry the panel's route through so the editor targets the right endpoint
  // even when hosting on a different-format example.
  editedRoute.value = route || null
  editedSourcePending = true
}

watch(selectedFormat, () => {
  if (!formatExamples.value.some((example) => example.id === selectedId.value)) {
    selectedId.value = formatExamples.value[0]?.id || ''
  }
})

function onSelectExample(id) {
  selectedId.value = id
  editedSourcePending = false // a fresh example picks resets the override
}

onMounted(async () => {
  connectBridge()
  loadCatalog()
  try {
    const response = await fetch(catalogUrl)
    if (!response.ok) throw new Error(`catalog request returned ${response.status}`)
    examples.value = await response.json()
    selectFormat(examples.value.some((example) => example.format === 'd2') ? 'd2' : examples.value[0]?.format)
  } catch (error) {
    loadError.value = `Could not load the executable example catalog: ${error.message}`
  }
})
</script>

<template>
  <main class="shell">
    <aside class="sidebar">
      <a class="brand" href="../">kr0ki <span>playb00k</span></a>
      <p class="version-tag">v{{ appVersion }}</p>
      <nav class="view-tabs" aria-label="Playbook view">
        <button class="view-tab" :class="{ active: viewMode === 'gallery' }" data-testid="tab-gallery" @click="viewMode = 'gallery'">
          Gallery
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'planner' }" data-testid="tab-planner" @click="viewMode = 'planner'">
          Planner
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'editor' }" data-testid="tab-editor" @click="viewMode = 'editor'">
          Code Editor
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'storyb00k' }" data-testid="tab-agent" @click="viewMode = 'storyb00k'">
          Agent
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'setup' }" data-testid="tab-setup" @click="viewMode = 'setup'">
          Setup
        </button>
      </nav>
      <CatalogTree
        :catalog="catalog"
        :examples="examples"
        :selected-example-id="viewMode === 'editor' ? selectedId : ''"
        :suggested="gallery.suggested"
        @select="onTreeSelect"
      />
      <a class="docs-link" href="../">Generated API docs ↗</a>
    </aside>

    <section class="content">
      <header v-if="viewMode !== 'planner'" class="hero">
        <p class="eyebrow">IAC / CODE → PROCEDURAL DIAGRAM → KROKI → SVG / PNG</p>
        <h1>Diagram-as-code, rendered.</h1>
        <p v-if="viewMode === 'gallery'">
          Not sure which diagram fits? Ask the <a href="#planner" @click.prevent="viewMode = 'planner'">Planner</a>, or browse by intent below. "Test all" renders and cache-verifies
          every fixture, the same contract <code>just test-playbook</code> checks.
        </p>
        <p v-else-if="viewMode === 'editor'">
          Pick a format, paste or upload your own diagram-as-code, and render it.
        </p>
      </header>

      <PlannerView
        v-if="plannerMounted"
        v-show="viewMode === 'planner'"
        :catalog="catalog"
        :examples="examples"
        :agent-url="agentUrl"
        :planner-session="plannerSession"
        :ui-status="uiStatus"
        :suggested="gallery.suggested"
        :suggest-note="gallery.note"
        :selected-type-id="gallery.pick"
        @open-in-editor="openInEditor"
        @agent-handoff="agentHandoff"
        @show-in-gallery="showInGallery"
      />
      <p v-if="loadError" class="error">{{ loadError }}</p>
      <Setup
        v-else-if="viewMode === 'setup'"
        :renderer-url="rendererUrl"
        :agent-url="agentUrl"
        :lsp-urls="lspUrls"
        @update:lsp-urls="updateLspUrls"
        @update:renderer-url="updateRendererUrl"
        @update:agent-url="updateAgentUrl"
        @update:llm-url="updateLlmUrl"
        @update:llm-key="updateLlmKey"
        @update:llm-model="updateLlmModel"
        @update:output-format="updateOutputFormat"
      />
      <Gallery
        v-else-if="viewMode === 'gallery'"
        :examples="examples"
        :agent-url="agentUrl"
        :renderer-url="rendererUrl"
        v-model:use-case="gallery.useCase"
        v-model:selected-type-id="gallery.selected"
        :suggested="gallery.suggested"
        :suggest-note="gallery.note"
        :catalog-data="catalog"
        @open-in-editor="openInEditor"
        @agent-handoff="agentHandoff"
      />
      <StoryB00k v-else-if="viewMode === 'storyb00k'" :prefill="agentPrefill" :locked-type="agentTypeId" :llm-url="llmUrl" :llm-key="llmKey" :llm-model="llmModel" :agent-url="agentUrl" :editor-handoff="editorHandoff" @edit-in-editor="editInEditor" />
      <RendererPanel
        v-else-if="selectedExample"
        :example="selectedExample"
        :examples="formatExamples"
        :override-source="editedSourcePending ? editedSource : undefined"
        :override-route="editedSourcePending ? editedRoute : undefined"
        :renderer-url="rendererUrl"
        :agent-url="agentUrl"
        :lsp-urls="lspUrls"
        :output-format="outputFormat"
        @select-example="onSelectExample"
        @send-to-agent="editorToAgentHandoff"
      />
      <p v-else class="loading">Loading test-backed examples…</p>
    </section>
  </main>
</template>
