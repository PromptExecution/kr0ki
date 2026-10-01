<script setup>
import { computed, nextTick, ref, watch } from 'vue'

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
  // The diagram-type catalog (/api/catalog), loaded once by the app and shared with the sidebar tree and the planner.
  catalogData: { type: Object, default: null },
})
const emit = defineEmits(['open-in-editor', 'agent-handoff', 'update:useCase', 'update:selectedTypeId'])

const rendererBase = computed(() => (props.rendererUrl || window.location.origin).trim())

// ---- Plan 005: intent-first catalog ----------------------------------------
// The taxonomy is Rust-owned (/api/catalog) and loaded by the app. The filter defaults to
// "All" and auto-resets to "All" whenever a selection stops matching.
const catalog = computed(() => props.catalogData)
const activeUseCase = computed({
  get: () => props.useCase,
  set: (value) => emit('update:useCase', value),
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
// Every catalog type with its fixture, regardless of the intent filter ("Test all" must cover the whole catalog).
const allTypeCards = computed(() =>
  (catalog.value?.types || []).map((t) => ({
    ...t,
    example: props.examples.find((e) => e.id === t.exampleId),
  })),
)
const typeCards = computed(() =>
  allTypeCards.value.filter((t) => activeUseCase.value === 'All' || t.useCases.includes(activeUseCase.value)),
)

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

// Summary covers BOTH grids: the type cards (top) and the raw fixtures (bottom).
const summary = computed(() => {
  const done = (v) => v === 'pass' || v === 'fail'
  const testableCards = allTypeCards.value.filter((c) => c.example)
  const cardsTested = testableCards.filter((c) => done(typeStatus.value[c.id]))
  const examplesTested = props.examples.filter((e) => done(status.value[e.id]))
  const passed =
    cardsTested.filter((c) => typeStatus.value[c.id] === 'pass').length +
    examplesTested.filter((e) => status.value[e.id] === 'pass').length
  return {
    tested: cardsTested.length + examplesTested.length,
    passed,
    total: testableCards.length + props.examples.length,
  }
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
  // The type cards first (they are what the user sees first), then every raw fixture.
  for (const card of allTypeCards.value) {
    // eslint-disable-next-line no-await-in-loop
    await testType(card)
  }
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
  <section class="gallery">
   <div class="gallery-main">
    <div class="controls gallery-controls">
      <button :disabled="running || (examples.length === 0 && !allTypeCards.length)" @click="testAll">
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

  </section>
</template>
