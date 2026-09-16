<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  examples: { type: Array, required: true },
})
const emit = defineEmits(['open-in-editor'])

const rendererUrl = ref(
  window.location.port === '8787'
    ? window.location.origin
    : new URLSearchParams(window.location.search).get('renderer') || '',
)

// Per-example-id maps: status is 'idle' | 'testing' | 'pass' | 'fail'.
const status = ref({})
const artifactUrls = ref({})
const errors = ref({})
const running = ref(false)

const summary = computed(() => {
  const tested = props.examples.filter((example) => status.value[example.id] === 'pass' || status.value[example.id] === 'fail')
  const passed = tested.filter((example) => status.value[example.id] === 'pass')
  return { tested: tested.length, passed: passed.length, total: props.examples.length }
})

function endpointFor(example, output) {
  return rendererUrl.value.trim()
    ? `${rendererUrl.value.trim().replace(/\/$/, '')}/render/${example.format}?output=${output}`
    : ''
}

async function testOne(example) {
  if (!rendererUrl.value.trim()) {
    status.value = { ...status.value, [example.id]: 'fail' }
    errors.value = { ...errors.value, [example.id]: 'Enter a network-reachable kr0ki URL first' }
    return
  }
  status.value = { ...status.value, [example.id]: 'testing' }
  errors.value = { ...errors.value, [example.id]: '' }
  try {
    let firstArtifact = null
    // Every declared output must render; keep the first as the card's thumbnail.
    for (const output of example.outputs) {
      const response = await fetch(endpointFor(example, output), {
        method: 'POST',
        headers: { 'Content-Type': 'text/plain' },
        body: example.source,
      })
      const bytes = await response.blob()
      if (!response.ok) throw new Error(`${output}: ${await bytes.text()}`)
      if (!firstArtifact) firstArtifact = URL.createObjectURL(bytes)
    }
    artifactUrls.value = { ...artifactUrls.value, [example.id]: firstArtifact }
    status.value = { ...status.value, [example.id]: 'pass' }
  } catch (error) {
    errors.value = { ...errors.value, [example.id]: error.message }
    status.value = { ...status.value, [example.id]: 'fail' }
  }
}

async function testAll() {
  running.value = true
  for (const example of props.examples) {
    // Sequential on purpose: a first pass exercises cache miss-then-hit behaviour
    // per example without racing the same renderer URL with concurrent requests.
    // eslint-disable-next-line no-await-in-loop
    await testOne(example)
  }
  running.value = false
}
</script>

<template>
  <section class="gallery">
    <div class="controls gallery-controls">
      <label>
        Renderer URL
        <input v-model="rendererUrl" aria-label="Renderer URL" placeholder="http://kr0ki-host:8787" />
      </label>
      <button :disabled="running || examples.length === 0" @click="testAll">
        {{ running ? 'Testing…' : 'Test all' }}
      </button>
      <p v-if="summary.tested > 0" class="gallery-summary">
        {{ summary.passed }}/{{ summary.tested }} of {{ summary.total }} passed
      </p>
    </div>

    <div class="gallery-grid">
      <article v-for="example in examples" :key="example.id" class="gallery-card">
        <header>
          <p class="eyebrow">{{ example.format }} · {{ example.input_kind }}</p>
          <h3>{{ example.title }}</h3>
        </header>
        <p class="card-description">{{ example.description }}</p>
        <div class="card-preview">
          <img
            v-if="artifactUrls[example.id]"
            :src="artifactUrls[example.id]"
            :alt="`${example.title} rendered output`"
          />
          <p v-else class="card-empty">Not tested yet</p>
        </div>
        <footer>
          <span class="card-status" :class="status[example.id] || 'idle'">{{ status[example.id] || 'idle' }}</span>
          <button type="button" class="secondary" @click="emit('open-in-editor', example)">Edit</button>
          <button :disabled="status[example.id] === 'testing'" @click="testOne(example)">Test</button>
        </footer>
        <p v-if="errors[example.id]" class="card-error">{{ errors[example.id] }}</p>
      </article>
    </div>
  </section>
</template>
