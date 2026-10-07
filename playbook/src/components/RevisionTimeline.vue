<script setup>
// Revision history as a plain timeline: one row per state of the diagram, oldest first, with what changed (+/- lines against its
// parent), the tool call behind it, a snapshot of the picture, and the actions that matter: view, compare, restore, fork, download.
// Restoring only changes which revision is current; nothing is ever deleted, so every state stays recoverable.
import { computed, ref, watch } from 'vue'
import { diffLines, diffStats } from '../lib/lineDiff.js'
import { parentOf } from '../lib/revisionGraph.js'
import { downloadText, sourceFilename } from '../lib/renderSource.js'

const props = defineProps({
  graph: { type: Object, required: true },
  /** tool call id -> {name, ok}, to label the row with its outcome */
  toolCalls: { type: Object, default: () => ({}) },
  /** revision to open (e.g. from a tool call's "revision" link) */
  focusId: { type: String, default: '' },
})
const emit = defineEmits(['restore', 'fork'])

const viewId = ref(props.focusId)
watch(() => props.focusId, (id) => { if (id) viewId.value = id })
const compareId = ref('')

const rows = computed(() =>
  props.graph.nodes.map((n, i) => {
    const parent = parentOf(props.graph, n.id)
    const stats = parent ? diffStats(parent.source, n.source) : { added: n.source ? n.source.split('\n').length : 0, removed: 0 }
    return { n, i, stats, active: n.id === props.graph.activeId, tool: n.toolCallId ? props.toolCalls[n.toolCallId] : null }
  }),
)
const byId = (id) => props.graph.nodes.find((x) => x.id === id)
const viewed = computed(() => byId(viewId.value))
const compared = computed(() => byId(compareId.value))
// Compare the chosen revision to the one being viewed (or to the current one when only a compare target is set).
const diff = computed(() => {
  if (!compared.value) return []
  const base = viewed.value || byId(props.graph.activeId)
  return diffLines(compared.value.source, base.source)
})
const time = (iso) => new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
const kindLabel = { root: 'start', prompt: 'agent', edit: 'edit', fork: 'fork' }

function view(n) { viewId.value = viewId.value === n.id ? '' : n.id }
function compare(n) { compareId.value = compareId.value === n.id ? '' : n.id }
function download(n) { downloadText(sourceFilename(n.format, `revision-${n.id.slice(-6)}`), n.source) }
</script>

<template>
  <div class="timeline" data-testid="revision-timeline">
    <p v-if="graph.nodes.length < 2" class="timeline__empty">Only the starting state exists. Each render the agent makes, and each edit you apply, adds a revision here.</p>
    <ol class="timeline__list">
      <li v-for="r in rows" :key="r.n.id" class="timeline__row" :class="{ 'timeline__row--active': r.active, 'timeline__row--viewed': r.n.id === viewId }" :data-testid="`rev-${r.i}`">
        <div class="timeline__head">
          <span class="timeline__badge" :data-kind="r.n.kind">{{ kindLabel[r.n.kind] || r.n.kind }}</span>
          <strong class="timeline__label" :title="r.n.prompt || r.n.label">#{{ r.i }} {{ r.n.label }}</strong>
          <span v-if="r.active" class="timeline__current">current</span>
          <span class="timeline__time">{{ time(r.n.createdAt) }}</span>
        </div>
        <div class="timeline__meta">
          <span class="timeline__stat timeline__stat--add" title="lines added vs its parent">+{{ r.stats.added }}</span>
          <span class="timeline__stat timeline__stat--del" title="lines removed vs its parent">−{{ r.stats.removed }}</span>
          <span v-if="r.n.toolName" class="timeline__tool">{{ r.n.toolName }}<template v-if="r.tool"> · {{ r.tool.ok ? '✓ succeeded' : '✗ failed' }}</template></span>
          <span v-if="r.n.format" class="timeline__fmt">{{ r.n.format }}</span>
        </div>
        <div class="timeline__actions">
          <button type="button" data-testid="rev-view" @click="view(r.n)">{{ r.n.id === viewId ? 'Hide' : 'View' }}</button>
          <button type="button" data-testid="rev-compare" :class="{ on: r.n.id === compareId }" title="Show what differs between this revision and the one being viewed (or the current one)" @click="compare(r.n)">Compare</button>
          <button type="button" data-testid="rev-restore" :disabled="r.active || !r.n.source" title="Make this the current state. The agent's next request starts from it. Later revisions are kept." @click="emit('restore', r.n.id)">Restore</button>
          <button type="button" data-testid="rev-fork" title="Start a new branch from this revision" @click="emit('fork', r.n.id)">Fork</button>
          <button type="button" data-testid="rev-download" :disabled="!r.n.source" title="Download this revision's code" @click="download(r.n)">⬇ Code</button>
        </div>
        <div v-if="r.n.id === viewId" class="timeline__view" data-testid="rev-snapshot">
          <img v-if="r.n.rendered?.imageDataUrl" :src="r.n.rendered.imageDataUrl" alt="Snapshot of this revision" />
          <div v-else-if="r.n.rendered?.svg" class="timeline__svg" v-html="r.n.rendered.svg"></div>
          <p v-else class="timeline__empty">No picture was captured for this revision.</p>
          <pre class="timeline__src">{{ r.n.source || '(empty)' }}</pre>
        </div>
      </li>
    </ol>
    <section v-if="compared" class="timeline__diff" data-testid="rev-diff">
      <header>
        <strong>Compare</strong> #{{ graph.nodes.indexOf(compared) }} → #{{ graph.nodes.indexOf(viewed || byId(graph.activeId)) }}{{ viewed ? '' : ' (current)' }}
        <button type="button" @click="compareId = ''">Close</button>
      </header>
      <pre><template v-for="(l, i) in diff" :key="i"><span :class="`d-${l.op}`">{{ l.op === 'add' ? '+' : l.op === 'del' ? '−' : ' ' }} {{ l.text }}
</span></template></pre>
    </section>
  </div>
</template>

<style scoped>
.timeline { display: flex; flex-direction: column; gap: .5rem; font-size: .85rem; }
.timeline__list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: .4rem; }
.timeline__row { border: 1px solid #c8ced8; border-left: 4px solid #94a3b8; border-radius: 6px; padding: .45rem .6rem; }
.timeline__row--active { border-left-color: #2563eb; background: rgba(37, 99, 235, .07); }
.timeline__row--viewed { outline: 2px solid #94a3b8; }
.timeline__head, .timeline__meta, .timeline__actions { display: flex; flex-wrap: wrap; align-items: center; gap: .4rem; }
.timeline__label { flex: 1; min-width: 8rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.timeline__badge { font-size: .65rem; text-transform: uppercase; letter-spacing: .04em; padding: .05rem .4rem; border-radius: 999px; color: #fff; background: #64748b; }
.timeline__badge[data-kind='root'] { background: #2563eb; }
.timeline__badge[data-kind='prompt'] { background: #059669; }
.timeline__badge[data-kind='edit'] { background: #7c3aed; }
.timeline__badge[data-kind='fork'] { background: #d97706; }
.timeline__current { font-size: .7rem; font-weight: 700; color: #2563eb; }
.timeline__time { font-size: .72rem; opacity: .65; }
.timeline__stat { font-family: ui-monospace, monospace; font-size: .75rem; }
.timeline__stat--add { color: #15803d; }
.timeline__stat--del { color: #b91c1c; }
.timeline__tool, .timeline__fmt { font-size: .72rem; opacity: .8; }
.timeline__actions button { font-size: .75rem; padding: .1rem .5rem; cursor: pointer; }
.timeline__actions button.on { font-weight: 700; }
.timeline__view { margin-top: .4rem; display: flex; flex-direction: column; gap: .4rem; }
.timeline__svg, .timeline__view img { max-width: 100%; overflow: auto; }
.timeline__svg :deep(svg) { max-width: 100%; height: auto; }
.timeline__src, .timeline__diff pre { margin: 0; padding: .4rem; background: rgba(100, 116, 139, .12); border-radius: 4px; white-space: pre-wrap; overflow-x: auto; max-height: 14rem; overflow-y: auto; font-size: .78rem; }
.timeline__diff { border: 1px solid #c8ced8; border-radius: 6px; padding: .45rem .6rem; }
.timeline__diff header { display: flex; gap: .5rem; align-items: center; margin-bottom: .3rem; }
.d-add { color: #15803d; background: rgba(21, 128, 61, .12); }
.d-del { color: #b91c1c; background: rgba(185, 28, 28, .12); }
.timeline__empty { opacity: .7; margin: 0; }
</style>
