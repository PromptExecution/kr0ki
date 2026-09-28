<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import RendererPanel from './components/RendererPanel.vue'
import Gallery from './components/Gallery.vue'
import StoryB00k from './components/StoryB00k.vue'
import Setup from './components/Setup.vue'

// Version from Cargo.toml (injected at build time by Vite)
const appVersion = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '0.0.0'

const examples = ref([])
const selectedFormat = ref('d2')
const selectedId = ref('')
const loadError = ref('')
const viewMode = ref('gallery')
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
  // Build a prompt that includes the diagram source and detected type
  const typeLabel = detectedType !== 'unknown' ? detectedType : format
  const hasImage = imageData ? ' and rendered image' : ''
  agentPrefill.value = `I have a ${typeLabel} diagram called "${title}". Please review the diagram source${hasImage} and suggest improvements.

Here's the diagram source:
\`\`\`
${source}
\`\`\`

Detected type: ${detectedType}
Format: ${format}
Output: ${output}`
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
      <p class="sidebar-copy">Example fixtures for every supported format — or clear the source and render your own.</p>
      <nav class="view-tabs" aria-label="Playbook view">
        <button class="view-tab" :class="{ active: viewMode === 'gallery' }" @click="viewMode = 'gallery'">
          Gallery
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'editor' }" @click="viewMode = 'editor'">
          Code Editor
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'storyb00k' }" @click="viewMode = 'storyb00k'">
          Agent
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'setup' }" @click="viewMode = 'setup'">
          Setup
        </button>
      </nav>
      <nav v-if="viewMode === 'editor'" aria-label="Supported diagram formats">
        <button
          v-for="format in formats"
          :key="format"
          class="format-link"
          :class="{ active: selectedFormat === format }"
          @click="selectFormat(format)"
        >
          {{ format }}
        </button>
      </nav>
      <nav v-else-if="examples.length" class="catalog-nav" aria-label="Example catalog">
        <p class="catalog-heading">Catalog · {{ examples.length }} fixtures</p>
        <button
          v-for="example in examples"
          :key="example.id"
          class="catalog-link"
          :title="example.description"
          @click="openInEditor(example)"
        >
          <span class="catalog-format">{{ example.format }}</span>
          <span class="catalog-title">{{ example.title }}</span>
        </button>
      </nav>
      <a class="docs-link" href="../">Generated API docs ↗</a>
    </aside>

    <section class="content">
      <header class="hero">
        <p class="eyebrow">IAC / CODE → PROCEDURAL DIAGRAM → KROKI → SVG / PNG</p>
        <h1>Diagram-as-code, rendered.</h1>
        <p v-if="viewMode === 'gallery'">
          Every format's fixture in one grid. Point it at a running kr0ki service and hit
          "Test all" to render and cache-verify the full catalog — the same contract
          <code>just test-playbook</code> checks, from the browser.
        </p>
        <p v-else-if="viewMode === 'editor'">
          Pick a format, paste or upload your own diagram-as-code, and render it.
        </p>
      </header>

      <p v-if="loadError" class="error">{{ loadError }}</p>
      <Setup
        v-else-if="viewMode === 'setup'"
        :renderer-url="rendererUrl"
        :agent-url="agentUrl"
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
        :output-format="outputFormat"
        @select-example="onSelectExample"
        @send-to-agent="editorToAgentHandoff"
      />
      <p v-else class="loading">Loading test-backed examples…</p>
    </section>
  </main>
</template>
