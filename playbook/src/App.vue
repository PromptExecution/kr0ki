<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import RendererPanel from './components/RendererPanel.vue'

const examples = ref([])
const selectedFormat = ref('d2')
const selectedId = ref('')
const loadError = ref('')

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
      <p class="sidebar-copy">Executable examples from the same catalog used by Rust tests and mdb00k.</p>
      <nav aria-label="Supported diagram formats">
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
      <a class="docs-link" href="../">Generated API docs ↗</a>
    </aside>

    <section class="content">
      <header class="hero">
        <p class="eyebrow">IAC / CODE → PROCEDURAL DIAGRAM → KROKI → SVG / PNG</p>
        <h1>Evaluate kr0ki with executable examples.</h1>
        <p>Choose a supported input format, inspect its fixture, render it through kr0ki, and review cache-backed output. D2 code flows are executable today; the typed SysML v2 path remains upstream of this renderer.</p>
      </header>

      <p v-if="loadError" class="error">{{ loadError }}</p>
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
