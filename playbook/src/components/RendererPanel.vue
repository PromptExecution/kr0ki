<script setup>
import { computed, ref, watch } from 'vue'

const props = defineProps({
  example: { type: Object, required: true },
  examples: { type: Array, required: true },
})
const emit = defineEmits(['select-example'])

const source = ref(props.example.source)
const output = ref(props.example.outputs[0])
const rendererUrl = ref(
  window.location.port === '8787'
    ? window.location.origin
    : new URLSearchParams(window.location.search).get('renderer') || '',
)
const artifactUrl = ref('')
const result = ref('Ready')
const busy = ref(false)

watch(
  () => props.example,
  (example) => {
    source.value = example.source
    output.value = example.outputs[0]
    artifactUrl.value = ''
    result.value = 'Ready'
  },
)

const renderEndpoint = computed(() =>
  rendererUrl.value.trim()
    ? `${rendererUrl.value.trim().replace(/\/$/, '')}/render/${props.example.format}?output=${output.value}`
    : '',
)

async function render() {
  busy.value = true
  artifactUrl.value = ''
  result.value = 'Rendering…'
  try {
    if (!renderEndpoint.value) throw new Error('Enter a network-reachable kr0ki URL first')
    const response = await fetch(renderEndpoint.value, {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: source.value,
    })
    const bytes = await response.blob()
    if (!response.ok) throw new Error(await bytes.text())
    artifactUrl.value = URL.createObjectURL(bytes)
    result.value = `${response.headers.get('x-kr0ki-cache') || 'miss'} · ${response.headers.get('x-kr0ki-key') || 'no cache key'}`
  } catch (error) {
    result.value = `Render failed: ${error.message}`
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <article class="panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">{{ example.format }} · {{ example.input_kind }}</p>
        <h2>{{ example.title }}</h2>
        <p>{{ example.description }}</p>
      </div>
      <label>
        Example
        <select :value="example.id" @change="emit('select-example', $event.target.value)">
          <option v-for="item in examples" :key="item.id" :value="item.id">{{ item.title }}</option>
        </select>
      </label>
    </div>

    <div class="pipeline" aria-label="Render pipeline">
      <span>{{ example.input_kind }}</span><b>→</b><span>procedural diagram</span><b>→</b><span>Kroki</span><b>→</b><span>SVG / PNG</span>
    </div>

    <div class="controls">
      <label>
        Renderer URL
        <input v-model="rendererUrl" aria-label="Renderer URL" placeholder="http://kr0ki-host:8787" />
      </label>
      <label>
        Output
        <select v-model="output">
          <option v-for="kind in example.outputs" :key="kind" :value="kind">{{ kind.toUpperCase() }}</option>
        </select>
      </label>
      <button :disabled="busy" @click="render">{{ busy ? 'Rendering…' : 'Render example' }}</button>
    </div>

    <div class="workspace">
      <label class="source-label">
        Test-backed source
        <textarea v-model="source" spellcheck="false" />
      </label>
      <section class="preview" aria-live="polite">
        <p class="status">{{ result }}</p>
        <img v-if="artifactUrl" :src="artifactUrl" :alt="`${example.title} rendered output`" />
        <p v-else class="empty">Render the fixture to inspect its artifact here.</p>
      </section>
    </div>
  </article>
</template>
