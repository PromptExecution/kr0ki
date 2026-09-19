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

watch(selectedFormat, () => {
  if (!formatExamples.value.some((example) => example.id === selectedId.value)) {
    selectedId.value = formatExamples.value[0]?.id || ''
  }
})

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
      <p class="sidebar-copy">Test-backed examples from the same catalog used by Rust tests and mdb00k — or clear the source and render your own diagram in any format below.</p>
      <nav class="view-tabs" aria-label="Playbook view">
        <button class="view-tab" :class="{ active: viewMode === 'gallery' }" @click="viewMode = 'gallery'">
          Gallery
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'editor' }" @click="viewMode = 'editor'">
          Editor
        </button>
        <button class="view-tab" :class="{ active: viewMode === 'storyb00k' }" @click="viewMode = 'storyb00k'">
          storyb00k
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
        <h1>Evaluate kr0ki with executable examples — or your own diagrams.</h1>
        <p v-if="viewMode === 'gallery'">
          Every format's fixture in one grid. Point it at a running kr0ki service and hit
          "Test all" to render and cache-verify the full catalog — the same contract
          <code>just test-playbook</code> checks, from the browser.
        </p>
        <p v-else-if="viewMode === 'editor'">
          Choose a supported input format, inspect its fixture, render it through kr0ki, and review cache-backed output. Every fixture's source is editable in place: paste or upload your own hand-authored diagram-as-code and render it the same way, independent of whether it came from a generator. D2 code flows are executable today; the typed SysML v2 path remains upstream of this renderer.
        </p>
      </header>

      <p v-if="loadError" class="error">{{ loadError }}</p>
      <Gallery v-else-if="viewMode === 'gallery'" :examples="examples" @open-in-editor="openInEditor" />
      <StoryB00k v-else-if="viewMode === 'storyb00k'" />
      <RendererPanel
        v-else-if="selectedExample"
        :example="selectedExample"
        :examples="formatExamples"
        @select-example="selectedId = $event"
      />
      <p v-else class="loading">Loading test-backed examples…</p>
    </section>
  </main>
</template>
