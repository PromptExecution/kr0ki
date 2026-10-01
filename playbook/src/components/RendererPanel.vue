<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { fetchSkill } from '../lib/skills.js'

const props = defineProps({
  example: { type: Object, required: true },
  examples: { type: Array, required: true },
  // Preloaded source handed over from StoryB00k's EDIT button. Applied once,
  // then the user owns the textarea. overrideRoute carries a custom endpoint
  // (e.g. /render/k8s-topology) when the source isn't a plain /render/{format}.
  overrideSource: { type: String, default: undefined },
  overrideRoute: { type: String, default: undefined },
  // Renderer URL from Setup (persisted to localStorage). Falls back to query param or current origin.
  rendererUrl: { type: String, default: '' },
  // The diagram agent, which serves the syntax skill shown under the source.
  agentUrl: { type: String, default: '' },
  // Output format from Setup (persisted to localStorage). Defaults to 'svg'.
  outputFormat: { type: String, default: 'svg' },
})
const emit = defineEmits(['select-example', 'send-to-agent'])

const source = ref(props.overrideSource ?? props.example.source)
// Use outputFormat from Setup, but respect example.outputs if the format doesn't support the chosen output
const getInitialOutput = () => {
  const preferred = props.outputFormat || 'svg'
  const available = props.example.outputs || ['svg']
  return available.includes(preferred) ? preferred : available[0]
}
const output = ref(getInitialOutput())
// Use prop if provided, otherwise fall back to query param or current hostname:8787
const getFallbackUrl = () => {
  if (props.rendererUrl) return props.rendererUrl
  const queryParam = new URLSearchParams(window.location.search).get('renderer')
  if (queryParam) return queryParam
  if (window.location.port === '8787') return window.location.origin
  return `${window.location.protocol}//${window.location.hostname}:8787`
}
const localRendererUrl = ref(getFallbackUrl())
const artifactUrl = ref('')
const result = ref('Ready')
const busy = ref(false)
const autoRender = ref(true)
// The syntax skill for this format, shown under the source (what the agent reads before it renders).
const skill = ref({ status: 'loading' })
let skillRequest = 0
async function loadSkill() {
  const mine = ++skillRequest
  skill.value = { status: 'loading' }
  const result = await fetchSkill(props.agentUrl, props.example.format)
  if (mine === skillRequest) skill.value = result // ignore a slow answer for a format we have already left
}
watch(() => [props.example.format, props.agentUrl], loadSkill, { immediate: true })
const handoffBusy = ref(false)
const handoffToast = ref(null) // { type: 'success'|'error', message: string }

// Watch for prop changes from Setup
watch(
  () => props.rendererUrl,
  (newVal) => {
    if (newVal) {
      localRendererUrl.value = newVal
    }
  },
)

watch(
  () => props.outputFormat,
  (newVal) => {
    const available = props.example.outputs || ['svg']
    if (newVal && available.includes(newVal)) {
      output.value = newVal
    }
  },
)

const outputChoices = computed(() => props.example.outputs)

watch(
  () => props.example,
  (example) => {
    source.value = props.overrideSource ?? example.source
    const available = example.outputs || ['svg']
    output.value = available.includes(props.outputFormat) ? props.outputFormat : available[0]
    artifactUrl.value = ''
    result.value = 'Ready'
  },
)

// A newly handed-over EDIT source (e.g. second EDIT click without an example
// change) replaces the textarea content once.
watch(
  () => props.overrideSource,
  (value) => {
    if (value !== undefined) {
      source.value = value
      artifactUrl.value = ''
      result.value = 'Ready'
    }
  },
)

// Auto-render: when source/output/endpoint changes and autoRender is on, re-render.
let autoRenderTimer = null
watch(
  () => [source.value, output.value, localRendererUrl.value],
  () => {
    if (!autoRender.value || busy.value) return
    if (autoRenderTimer) clearTimeout(autoRenderTimer)
    autoRenderTimer = setTimeout(() => render(), 500)
  },
)

// Auto-render on mount when autoRender is enabled
onMounted(() => {
  if (autoRender.value && source.value) {
    render()
  }
})

function resetToExample() {
  source.value = props.example.source
  artifactUrl.value = ''
  result.value = 'Ready'
}

function clearSource() {
  source.value = ''
  artifactUrl.value = ''
  result.value = 'Ready'
}

function onFileSelected(event) {
  const file = event.target.files?.[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = () => {
    source.value = String(reader.result ?? '')
    artifactUrl.value = ''
    result.value = `Loaded ${file.name}`
  }
  reader.onerror = () => {
    result.value = `Could not read ${file.name}: ${reader.error?.message ?? 'unknown error'}`
  }
  reader.readAsText(file)
  event.target.value = ''
}

const renderEndpoint = computed(() => {
  if (!localRendererUrl.value.trim()) return ''
  // See Gallery.vue's endpointFor: a custom-route example (e.g.
  // POST /render/k8s-topology) isn't reachable via /render/{format}.
  // An EDIT override may carry its own route (agent k8s renders hosted on a
  // different-format example must still hit their own endpoint).
  const path = props.overrideRoute || props.example.route || `/render/${props.example.format}`
  return `${localRendererUrl.value.trim().replace(/\/$/, '')}${path}?output=${output.value}`
})

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

// Diagram type detection (best-guess regex)
function detectDiagramType(code) {
  if (!code) return 'unknown'
  const trimmed = code.trim()
  
  // D2: starts with declarations or has D2-specific syntax
  if (/^\s*\w+\s*:\s*\{/m.test(trimmed) || /^\s*\w+\s*->\s*\w+/m.test(trimmed)) {
    // Check for PlantUML markers first (more specific)
    if (/^@startuml/m.test(trimmed)) return 'plantuml'
    return 'd2'
  }
  
  // PlantUML
  if (/^@startuml/m.test(trimmed) || /@enduml/.test(trimmed)) return 'plantuml'
  
  // C4PlantUML (subset of PlantUML with C4 keywords)
  if (/C4_Context|C4Person|C4Container|C4Component|C4Deployment/i.test(trimmed)) return 'c4plantuml'
  
  // Mermaid
  if (/^(graph|sequenceDiagram|classDiagram|stateDiagram|erDiagram|flowchart|gantt|pie|gitGraph)/m.test(trimmed)) return 'mermaid'
  
  // Graphviz/DOT
  if (/^(digraph|graph|strict digraph|strict graph)\s*\{/m.test(trimmed)) return 'graphviz'
  
  // Structurizr DSL
  if (/^workspace\s*\{/m.test(trimmed) || /^model\s*\{/m.test(trimmed)) return 'structurizr'
  
  // Nomnoml
  if (/^#\[.*\]/m.test(trimmed) || /\[.*\|.*\]/.test(trimmed)) return 'nomnoml'
  
  // Erd (Entity Relationship)
  if (/^\s*\w+\s*\{[^}]*\}/m.test(trimmed) && /\s+\w+\s+\w+/m.test(trimmed)) return 'erd'
  
  // SVG (already rendered)
  if (/^<\?xml.*<svg/m.test(trimmed) || /^<svg/m.test(trimmed)) return 'svg'
  
  // BPMN (XML-based)
  if (/<bpmn:/m.test(trimmed) || /definitions.*bpmn/m.test(trimmed)) return 'bpmn'
  
  // Bytefield
  if (/^\s*\(defdsl\s/m.test(trimmed) || /\(entry\s/m.test(trimmed)) return 'bytefield'
  
  // Pikchr (Tcl-like)
  if (/^\s*(box|line|arrow|circle)\s/m.test(trimmed)) return 'pikchr'
  
  // WaveDrom
  if (/^\s*\{\s*"signal"/m.test(trimmed) || /"reg"\s*:/m.test(trimmed)) return 'wavedrom'
  
  // K8s (YAML with k8s-specific fields)
  if (/^apiVersion:\s*v1$/m.test(trimmed) || /kind:\s*(Pod|Service|Deployment|ConfigMap|Namespace)/m.test(trimmed)) return 'k8s'
  
  return 'unknown'
}

async function sendToAgent() {
  if (!source.value?.trim()) {
    handoffToast.value = { type: 'error', message: 'Cannot send empty diagram source' }
    setTimeout(() => { if (handoffToast.value?.type === 'error') handoffToast.value = null }, 4000)
    return
  }
  handoffBusy.value = true
  handoffToast.value = null
  const detectedType = detectDiagramType(source.value)
  
  // Convert rendered image to base64 data URI
  let imageData = null
  if (artifactUrl.value) {
    try {
      const response = await fetch(artifactUrl.value)
      const blob = await response.blob()
      const reader = new FileReader()
      imageData = await new Promise((resolve, reject) => {
        reader.onload = () => resolve(reader.result)
        reader.onerror = reject
        reader.readAsDataURL(blob)
      })
    } catch (error) {
      console.warn('Failed to capture rendered image:', error)
    }
  }
  
  const meta = {
    source: source.value,
    format: props.example.format,
    detectedType,
    output: output.value,
    imageData,
    title: props.example.title,
  }
  console.info('[renderer] handoff →', { format: meta.format, detectedType, title: meta.title, sourceLen: meta.source.length })
  emit('send-to-agent', meta)
  handoffToast.value = { type: 'success', message: `Sent ${detectedType} diagram (${meta.source.length} chars) to Agent` }
  handoffBusy.value = false
  setTimeout(() => { handoffToast.value = null }, 5000)
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
      <button :disabled="busy" @click="render">{{ busy ? 'Rendering…' : 'Render' }}</button>
      <label class="auto-render">
        <input type="checkbox" v-model="autoRender" />
        Auto Render
      </label>
      <button type="button" class="send-to-agent" :disabled="handoffBusy" @click="sendToAgent" title="Send diagram to Agent for collaborative editing">
        <span v-if="handoffBusy" class="handoff-spinner"></span>
        {{ handoffBusy ? 'Sending…' : 'Send to Agent' }}
      </button>
      <Transition name="toast">
        <span v-if="handoffToast" class="handoff-toast" :data-type="handoffToast.type">{{ handoffToast.message }}</span>
      </Transition>
      <button type="button" class="secondary" @click="resetToExample">Reset to example</button>
      <button type="button" class="secondary" @click="clearSource">Start blank</button>
      <label class="upload">
        Upload file
        <input type="file" aria-label="Upload a diagram source file" @change="onFileSelected" />
      </label>
    </div>

    <div class="workspace">
      <div class="editor-col">
        <label class="source-label">
          Diagram source — edit, paste, or upload your own {{ example.format }} source; not
          limited to the example shown
          <textarea v-model="source" spellcheck="false" data-testid="source-editor" />
        </label>
        <details class="skill-panel" open data-testid="skill-panel">
          <summary>Syntax skill · {{ example.format }} <span class="skill-hint">what the agent reads before it renders this language</span></summary>
          <pre v-if="skill.status === 'ok'" class="skill-text" data-testid="skill-text">{{ skill.text }}</pre>
          <p v-else-if="skill.status === 'loading'" class="skill-note">Loading the {{ example.format }} skill…</p>
          <p v-else-if="skill.status === 'missing'" class="skill-note" data-testid="skill-missing">No syntax skill for {{ example.format }} yet.</p>
          <p v-else class="skill-note" data-testid="skill-error">Skill unavailable ({{ skill.message }}). Is the agent running at the configured URL?</p>
        </details>
      </div>
      <section class="preview" aria-live="polite">
        <p class="status">{{ result }}</p>
        <img v-if="artifactUrl" :src="artifactUrl" :alt="`${example.title} rendered output`" />
        <p v-else class="empty">Render the fixture to inspect its artifact here.</p>
      </section>
    </div>
  </article>
</template>

<style scoped>
.handoff-spinner {
  display: inline-block; width: .85rem; height: .85rem;
  border: 2px solid currentColor; border-top-color: transparent;
  border-radius: 50%; animation: handoff-spin .6s linear infinite;
  vertical-align: middle; margin-right: .3rem;
}
@keyframes handoff-spin { to { transform: rotate(360deg); } }
.handoff-toast {
  display: inline-block; font-size: .78rem; padding: .2rem .6rem;
  border-radius: .35rem; margin-left: .5rem; vertical-align: middle;
}
.handoff-toast[data-type='success'] { background: #065f46; color: #a7f3d0; }
.handoff-toast[data-type='error'] { background: #7f1d1d; color: #fecaca; }
.toast-enter-active { transition: opacity .25s ease, transform .25s ease; }
.toast-leave-active { transition: opacity .4s ease; }
.toast-enter-from { opacity: 0; transform: translateY(-4px); }
.toast-leave-to { opacity: 0; }
</style>

