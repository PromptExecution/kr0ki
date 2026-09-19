<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import RendererPanel from './components/RendererPanel.vue'
import Gallery from './components/Gallery.vue'
import StoryB00k from './components/StoryB00k.vue'

const examples = ref([])
const selectedFormat = ref('d2')
const selectedId = ref('')
const loadError = ref('')
const viewMode = ref('gallery')
// Source handed over from StoryB00k's EDIT button (agent-rendered diagram).
const editedSource = ref('')
const editedRoute = ref(null)
let editedSourcePending = false

// Gallery → Agent handoff (Plan 005 §1.3): prefill the Agent composer with a
// prompt naming the diagram type, then flip to the Agent view. Never auto-sent.
const agentUrl = `${window.location.protocol}//${window.location.hostname}:8789`
const agentPrefill = ref('')

function agentHandoff({ prompt }) {
  agentPrefill.value = prompt
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
      <p class="sidebar-copy">Example fixtures for every supported format — or clear the source and render your own.</p>
      <nav class="view-tabs" aria-label="Playbook view">
        <button class="view-tab" :class="{ active: viewMode === 'gallery' }" @click="viewMode = 'gallery'">
          Gallery
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'editor' }" @click="viewMode = 'editor'">
          Editor
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'storyb00k' }" @click="viewMode = 'storyb00k'">
          Agent
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
      <Gallery
        v-else-if="viewMode === 'gallery'"
        :examples="examples"
        :agent-url="agentUrl"
        @open-in-editor="openInEditor"
        @agent-handoff="agentHandoff"
      />
      <StoryB00k v-else-if="viewMode === 'storyb00k'" :prefill="agentPrefill" @edit-in-editor="editInEditor" />
      <RendererPanel
        v-else-if="selectedExample"
        :example="selectedExample"
        :examples="formatExamples"
        :override-source="editedSourcePending ? editedSource : undefined"
        :override-route="editedSourcePending ? editedRoute : undefined"
        @select-example="onSelectExample"
      />
      <p v-else class="loading">Loading test-backed examples…</p>
    </section>
  </main>
</template>
