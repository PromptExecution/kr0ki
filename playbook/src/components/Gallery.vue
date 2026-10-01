<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import StoryB00k from './StoryB00k.vue'

const props = defineProps({
  examples: { type: Array, required: true },
  agentUrl: { type: String, default: '' },
  // The kr0ki service to render against comes from Setup (one place to configure it), not from a field here.
  rendererUrl: { type: String, default: '' },
  // Gallery state is owned by the app so the planner (via the UI bridge) and the user steer the same thing.
  useCase: { type: String, default: 'All' },
  selectedTypeId: { type: String, default: '' },
  suggested: { type: Array, default: () => [] },
  suggestNote: { type: String, default: '' },
  plannerSession: { type: String, default: '' },
  uiStatus: { type: String, default: 'closed' },
})
const emit = defineEmits(['open-in-editor', 'agent-handoff', 'update:useCase', 'update:selectedTypeId'])

const rendererBase = computed(() => (props.rendererUrl || window.location.origin).trim())
const plannerOpen = ref(true)

// ---- Plan 005: intent-first catalog ----------------------------------------
// Loaded from /api/catalog (Rust-owned taxonomy). The filter defaults to
// "All" and auto-resets to "All" whenever a selection stops matching.
const catalog = ref(null)
const activeUseCase = computed({
  get: () => props.useCase,
  set: (value) => emit('update:useCase', value),
})

onMounted(async () => {
  try {
    const base = rendererBase.value
    const res = await fetch(`${base.replace(/\/$/, '')}/api/catalog`)
    if (res.ok) catalog.value = await res.json()
  } catch (err) {
    console.warn('[gallery] catalog unavailable:', err?.message)
  }
})

const useCaseFilters = computed(() => ['All', ...(catalog.value?.useCases || [])])

// Type cards from the taxonomy; each links to its fixture (match by syntax)
// so the card can render a thumbnail through the existing test flow.
const selectedType = computed({
  get: () => props.selectedTypeId,
  set: (value) => emit('update:selectedTypeId', value),
})

function selectType(typeId) {
  selectedType.value = typeId
  const url = new URL(window.location.href)
  url.searchParams.set('type', typeId)
  window.history.replaceState({}, '', url)
}

async function focusSelectedType() {
  await nextTick()
  const card = document.querySelector(`[data-type-id="${CSS.escape(selectedType.value)}"]`)
  card?.focus({ preventScroll: true })
  card?.scrollIntoView({ block: 'center', behavior: 'smooth' })
}

// Type cards link to the fixture explicitly named by their catalog type.
const typeCards = computed(() => {
  const types = catalog.value?.types || []
  return types
    .filter((t) => activeUseCase.value === 'All' || t.useCases.includes(activeUseCase.value))
    .map((t) => ({
      ...t,
      example: props.examples.find((e) => e.id === t.exampleId),
    }))
})

// Auto-select "All" when the active filter somehow stops matching (e.g.
// catalog reload) — the filter never dead-ends.
watch(useCaseFilters, (filters) => {
  if (!filters.includes(activeUseCase.value)) activeUseCase.value = 'All'
})

watch([catalog, selectedType], () => {
  const chosen = (catalog.value?.types || []).find((type) => type.id === selectedType.value)
  if (!chosen) return
  // A selection (e.g. from the planner) must never be hidden by the current filter.
  if (activeUseCase.value !== 'All' && !chosen.useCases.includes(activeUseCase.value)) activeUseCase.value = 'All'
  focusSelectedType()
}, { immediate: true })

const suggestedSet = computed(() => new Set(props.suggested))
const suggestedNames = computed(() =>
  props.suggested.map((id) => (catalog.value?.types || []).find((t) => t.id === id)?.name).filter(Boolean))

function countFor(tag) {
  return (catalog.value?.types || []).filter((t) => t.useCases.includes(tag)).length
}

function agentHandoff(card) {
  // Plan 005 §1.3: pre-populate the Agent composer with a prompt that names
  // the type explicitly. Never auto-sent.
  emit('agent-handoff', {
    typeId: card.id,
    syntax: card.syntax,
    prompt: card.samplePrompt,
  })
}

// ---- Existing test flow -----------------------------------------------------
// Status/artifacts are keyed per CARD (typeId), not per example id: ten
// PlantUML types share one fixture, so an example-id key would light up every
// PlantUML card when a single Test completes.
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
  if (!rendererBase.value) return ''
  // A custom-route example (e.g. POST /render/k8s-topology) isn't reachable
  // by templating `format` into /render/{format} -- its input isn't
  // diagram-format source text at all. Use the declared route verbatim.
  const path = example.route || `/render/${example.format}`
  return `${rendererBase.value.replace(/\/$/, '')}${path}?output=${output}`
}

async function testOne(example) {
  if (!rendererBase.value) {
    status.value = { ...status.value, [example.id]: 'fail' }
    errors.value = { ...errors.value, [example.id]: 'No kr0ki service URL is configured (see Setup)' }
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

// ---- Type-card test flow (Plan 005): same renderer, card-scoped keys --------
const typeStatus = ref({})
const typeArtifacts = ref({})
const typeErrors = ref({})

async function testType(card) {
  const example = card.example
  if (!example) return
  if (!rendererBase.value) {
    typeStatus.value = { ...typeStatus.value, [card.id]: 'fail' }
    typeErrors.value = { ...typeErrors.value, [card.id]: 'No kr0ki service URL is configured (see Setup)' }
    return
  }
  typeStatus.value = { ...typeStatus.value, [card.id]: 'testing' }
  typeErrors.value = { ...typeErrors.value, [card.id]: '' }
  try {
    let firstArtifact = null
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
    typeArtifacts.value = { ...typeArtifacts.value, [card.id]: firstArtifact }
    typeStatus.value = { ...typeStatus.value, [card.id]: 'pass' }
  } catch (error) {
    typeErrors.value = { ...typeErrors.value, [card.id]: error.message }
    typeStatus.value = { ...typeStatus.value, [card.id]: 'fail' }
  }
}
</script>

<template>
  <section class="gallery" :class="{ 'gallery--planner': plannerOpen }">
   <div class="gallery-main">
    <div class="controls gallery-controls">
      <button :disabled="running || examples.length === 0" @click="testAll">
        {{ running ? 'Testing…' : 'Test all' }}
      </button>
      <p v-if="summary.tested > 0" class="gallery-summary">
        {{ summary.passed }}/{{ summary.tested }} of {{ summary.total }} passed
      </p>
    </div>

    <!-- Planner shortlist: what the planning agent suggests, with its reason -->
    <aside v-if="suggestedNames.length" class="gallery-suggest" data-testid="planner-suggestion" role="status">
      <strong>★ Planner suggests:</strong> {{ suggestedNames.join(' · ') }}
      <span v-if="suggestNote" class="gallery-suggest-note"> — {{ suggestNote }}</span>
    </aside>
    <!-- Intent filter (Plan 005): defaults to All, auto-resets to All -->
    <nav v-if="useCaseFilters.length > 1" class="gallery-filters" aria-label="Filter by use case">
      <span class="filter-label">I want to show</span>
      <button
        v-for="tag in useCaseFilters"
        :key="tag"
        class="gallery-filter"
        :class="{ active: activeUseCase === tag }"
        :aria-pressed="activeUseCase === tag"
        @click="activeUseCase = tag"
      >{{ tag }}<span v-if="tag !== 'All'" class="count">{{ countFor(tag) }}</span></button>
    </nav>
    <p v-if="catalog && !typeCards.length" class="empty" style="padding: 0 1.5rem;">
      Nothing matches that filter yet.
    </p>

    <!-- Type cards: browse by intent, deep-linkable by typeId -->
    <div v-if="typeCards.length" class="gallery-grid">
      <article
        v-for="card in typeCards"
        :key="card.id"
        class="gallery-card"
        :class="{ selected: selectedType === card.id, suggested: suggestedSet.has(card.id) }"
        :data-type-id="card.id"
        tabindex="-1"
        @click="selectType(card.id)"
      >
        <header>
          <p class="eyebrow">
            <span v-if="suggestedSet.has(card.id)" class="badge-suggested" data-testid="suggested-badge">★ suggested</span>
            {{ card.name }} · {{ card.syntax }}
          </p>
          <h3>{{ card.blurb }}</h3>
        </header>
        <p class="card-description">{{ card.useCases.join(' · ') }}</p>
        <div class="card-preview">
          <img
            v-if="typeArtifacts[card.id]"
            :src="typeArtifacts[card.id]"
            :alt="`${card.name} rendered sample`"
          />
          <p v-else class="card-empty">Hit Test below to render a sample</p>
        </div>
        <footer>
          <button
            v-if="card.example"
            type="button"
            class="secondary"
            :disabled="typeStatus[card.id] === 'testing'"
            data-testid="card-test"
            @click="testType(card)"
          >{{ typeStatus[card.id] === 'testing' ? 'Testing…' : 'Test' }}</button>
          <button type="button" class="secondary" data-testid="card-edit" @click="emit('open-in-editor', card.example)">Edit</button>
          <button type="button" class="agent" data-testid="card-agent" :title="`Open the Agent with a ${card.name} prompt pre-filled`" @click="agentHandoff(card)">Agent</button>
        </footer>
        <p v-if="typeErrors[card.id]" class="card-error">{{ typeErrors[card.id] }}</p>
      </article>
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
   </div>

   <aside class="gallery-planner" aria-label="Diagram planner" data-testid="planner-panel">
    <header class="gallery-planner-head">
      <button type="button" class="secondary" :aria-expanded="plannerOpen" data-testid="planner-toggle" @click="plannerOpen = !plannerOpen">
        {{ plannerOpen ? '▾' : '▸' }} Diagram planner
      </button>
      <span class="planner-link" :data-status="uiStatus" :title="`UI session ${plannerSession}`">
        {{ uiStatus === 'open' ? '● page linked' : uiStatus === 'connecting' ? '○ linking…' : '○ not linked' }}
      </span>
    </header>
    <StoryB00k
      v-if="plannerOpen && plannerSession"
      planner
      :thread-id="plannerSession"
      :agent-url="agentUrl"
    />
    <p v-if="plannerOpen && plannerSession" class="planner-session" data-testid="planner-session">
      MCP clients can steer this page with <code>navigate_ui</code>, session <code>{{ plannerSession }}</code>
    </p>
   </aside>
  </section>
</template>
