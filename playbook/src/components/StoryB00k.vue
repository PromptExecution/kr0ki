<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { useChat } from '@synoped/ag-ui-vue'
import StoryB00kPanel from './StoryB00kPanel.vue'
import RevisionFlow from './RevisionFlow.vue'
import {
  createRevisionGraph, activeNode, addPromptNode, addEditNode,
  checkoutNode, forkFrom, serialize as serializeGraph,
} from '../lib/revisionGraph.js'

// Preserve the browser-visible host so LAN users reach this pod's sidecar instead
// of their own workstation's localhost.
const agentUrl = import.meta.env.VITE_STORYB00K_AGENT_URL || `${window.location.protocol}//${window.location.hostname}:8789`

// crypto.randomUUID() is secure-context-only (HTTPS or localhost). This pod
// serves plain HTTP on a LAN IP, so the browser hides it and the ag-ui client
// crashes calling it for run/message ids. window.crypto itself is read-only,
// so patch the prototype the instance inherits from — before useChat.
if (!globalThis.crypto?.randomUUID) {
  const proto = Object.getPrototypeOf(globalThis.crypto)
  const target = proto ?? globalThis.crypto
  Object.defineProperty(target, 'randomUUID', {
    value: () =>
      ([1e7] + -1e3 + -4e3 + -8e3 + -1e11).replace(/[018]/g, c =>
        (c ^ globalThis.crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> c / 4).toString(16)
      ),
    configurable: true,
  })
  console.warn('[storyb00k] crypto.randomUUID unavailable (insecure context) — installed prototype fallback')
}
const input = ref('')
const autoScroll = ref(true)
const transcriptEl = ref(null)
const chat = useChat({
  url: `${agentUrl}/run`,
  initialState: { panels: [], drafts: [] },
  // Our submitAnswer() drives continuation explicitly via chat.send(answer);
  // the client's auto-resume fires resume() the moment all interrupts have
  // responses, colliding with that send ("A run is already in progress").
  autoResumeInterrupts: false,
  // Pipe every AG-UI event and lifecycle transition to the browser console —
  // this is the debugger for the "UI feels disconnected" class of problem.
  debug: { events: true, lifecycle: true },
})

// `useChat` returns Vue refs. Expose the values themselves at the component
// boundary so Vue's template proxy can keep the controls and dashboard reactive.
// Accessing `chat.status` or `chat.state.panels` directly compares/dereferences
// the Ref object rather than its value, which disables Send and can crash the UI.
const items = chat.items
const interrupts = chat.interrupts
const status = chat.status
const error = chat.error
const toolCallTrackers = chat.toolCallTrackers
const usage = chat.usage
const threadId = chat.threadId
const panels = computed(() => chat.state.value?.panels || [])
const drafts = computed(() => chat.state.value?.drafts || [])
const busy = computed(() => status.value === 'submitted' || status.value === 'streaming')
const toolActivity = computed(() => Array.from(toolCallTrackers.value?.values() || []).map(t => ({
  id: t.toolCallId,
  name: t.toolName,
  running: t.state === 'input-streaming' || t.state === 'input-available' || t.state === 'approval-requested',
  failed: t.state === 'output-error' || t.state === 'output-denied',
  output: t.output,
})))
// ---- Project: the conceptual unit of work within a session ----
// The agent tracks goal/Q&A/prompts server-side (project_store.py); the UI
// mirrors it so the user can see (and edit) what was asked and answered.
const project = ref(null)
const projectError = ref('')
const editingTitle = ref(false)
const editedTitle = ref('')
const showPromptLog = ref(false)
const promptLog = computed(() => project.value?.prompts || [])
const thinkingLog = computed(() => project.value?.thinking || [])

async function loadProject() {
  try {
    const res = await fetch(`${agentUrl}/projects/${threadId.value}`)
    if (res.status === 404) { project.value = null; return }
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    project.value = await res.json()
  } catch (err) {
    projectError.value = err.message
  }
}

watch(threadId, () => { project.value = null; loadProject() })
loadProject()

async function renameProject() {
  const title = editedTitle.value.trim()
  editingTitle.value = false
  if (!title) return
  try {
    const res = await fetch(`${agentUrl}/projects/rename`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ threadId: threadId.value, title }),
    })
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    if (project.value) project.value.title = title
  } catch (err) {
    projectError.value = `rename failed: ${err.message}`
  }
}

// Refresh the project mirror after each run completes (new prompts/Q&A/thinking).
watch(status, (next, prev) => {
  if (prev !== 'ready' && next === 'ready') loadProject()
})

const statusLabel = computed(() => ({
  ready: 'Ready',
  submitted: 'Thinking…',
  streaming: 'Working…',
  error: 'Error',
}[status.value] || status.value))

// ---- Revision graph: every prompt/render is a node; time travel + forks ----
const revisionGraph = reactive(createRevisionGraph({ source: '', format: 'd2', label: 'Session start' }))
const showRevisions = ref(false)
const editingMessageId = ref(null)
const editedPrompt = ref('')
const activeRevision = computed(() => activeNode(revisionGraph))
const saveState = ref('')

// Watch panel renders: when the agent produces a new diagram, record it as a
// prompt node (the prompt that produced it) on top of the active revision.
watch(panels, (list) => {
  const latest = [...list].reverse().find((p) => p.kind === 'render' && p.source?.text)
  if (!latest) return
  if (revisionGraph.nodes.some((n) => n.source === latest.source.text && n.kind !== 'root')) return
  addPromptNode(revisionGraph, {
    prompt: latest.promptUsed || latest.toolName || 'agent render',
    source: latest.source.text,
    format: latest.source.format || 'd2',
    route: latest.source.route || null,
    notes: '',
  })
  saveState.value = ''
}, { deep: true })

async function persistChart(description) {
  try {
    const res = await fetch(`${agentUrl}/charts/save`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        threadId: threadId.value,
        graph: JSON.parse(serializeGraph(revisionGraph)),
        source: activeRevision.value.source,
        format: activeRevision.value.format,
        description,
      }),
    })
    const data = await res.json()
    saveState.value = data.jj ? `saved + jj snapshot ✓` : data.path ? `saved ✓ (plain file)` : `save failed: ${data.error || 'unknown'}`
  } catch (err) {
    saveState.value = `save failed: ${err.message}`
  }
}

function checkoutRevision(nodeId) {
  checkoutNode(revisionGraph, nodeId)
  console.info('[storyb00k] time travel →', nodeId)
  // Push the restored state into the live panel stream so the dashboard shows it.
  const node = activeNode(revisionGraph)
  if (node.source) {
    chat.state.value = { panels: [{ kind: 'render', toolName: 'time-travel', content: '', source: { text: node.source, format: node.format, route: node.route } }], drafts: drafts.value }
  }
}

function forkRevision(nodeId) {
  const node = forkFrom(revisionGraph, nodeId, 'fork')
  console.info('[storyb00k] forked from', nodeId, '→', node.id)
  persistChart(`fork from ${nodeId}`)
}

function startEditPrompt(item) {
  if (item.role !== 'user') return
  editingMessageId.value = item.id
  editedPrompt.value = item.parts.map((p) => (p.type === 'text' ? p.text : '')).join('')
}

function cancelEditPrompt() {
  editingMessageId.value = null
  editedPrompt.value = ''
}

// Regenerate: re-run an edited prompt from the ACTIVE revision's diagram state.
// The new node's source starts as the active state; the agent's next render
// (watched above) becomes the mutation on top.
async function regeneratePrompt(item, editedText) {
  const text = (editedText ?? editedPrompt.value).trim()
  if (!text || busy.value) return
  console.info('[storyb00k] regenerate from revision', activeRevision.value.id, '→', text)
  addPromptNode(revisionGraph, {
    prompt: text,
    source: activeRevision.value.source,
    format: activeRevision.value.format,
    route: activeRevision.value.route,
    notes: 'regenerating…',
  })
  editingMessageId.value = null
  editedPrompt.value = ''
  input.value = ''
  // Editing + resending means the user wants the agent to see the correction:
  // send as a fresh run — the agent's own tools render from the authoritative
  // model, and the watch above attaches its output to this revision node.
  try {
    await chat.send(text)
  } catch (err) {
    console.error('[storyb00k] regenerate failed:', err?.message ?? err)
  }
}

async function sendMessage() {
  const text = input.value.trim()
  if (!text || busy.value) return
  input.value = ''
  console.info('[storyb00k] send →', text, '| thread:', threadId.value, '| agent:', agentUrl)
  try {
    const result = await chat.send(text)
    console.info('[storyb00k] run finished | new messages:', result?.newMessages?.length ?? 0,
      '| usage:', usage.value.at(-1) ?? 'none reported')
  } catch (err) {
    console.error('[storyb00k] run failed:', err?.message ?? err)
  }
}

async function answerInterrupt(interrupt, approved) {
  console.info('[storyb00k] interrupt response →', interrupt.id, approved)
  await fetch(`${agentUrl}/respond-to-interrupt`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ threadId: threadId.value, interruptId: interrupt.id, approved }),
  })
  chat.respondToInterrupt(interrupt.id, { approved })
}

// Planning questions (ask_user interrupts): pick an option or type free text,
// then continue the run with the answer appended as a user message.
const questionChoice = ref('')
const questionFreeText = ref('')

function isQuestionInterrupt(interrupt) {
  return typeof interrupt.id === 'string' && interrupt.id.startsWith('ask-')
}

// zod strips unknown keys from interrupt objects, so the options ride in the
// responseSchema's `options.default` (defaults survive parsing).
function optionsFor(interrupt) {
  return interrupt.responseSchema?.properties?.options?.default || []
}

function startAnswer(interrupt) {
  // Kept for symmetry/test hooks; the answer UI renders inline now.
  questionChoice.value = ''
  questionFreeText.value = ''
  void interrupt
}

async function submitAnswer(interruptId) {
  const interrupt = interrupts.value.find((i) => i.id === interruptId)
  if (!interrupt) return
  const answer = questionFreeText.value.trim() || questionChoice.value
  if (!answer) return
  try {
    const res = await fetch(`${agentUrl}/respond-to-interrupt`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ threadId: threadId.value, interruptId: interrupt.id, answer }),
    })
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    questionChoice.value = ''
    questionFreeText.value = ''
    // Record the response with the client (no auto-resume — disabled), then
    // continue the planning loop explicitly with the answer as a user message.
    chat.respondToInterrupt(interrupt.id, { answer })
    await chat.send(answer)
    loadProject()
  } catch (err) {
    console.error('[storyb00k] answer failed:', err?.message ?? err)
    projectError.value = `answer failed: ${err.message}`
  }
}

function decideDraft(draft, approved) {
  // Mirror the interrupt flow for drafts surfaced in state (e.g. after reconnect).
  return fetch(`${agentUrl}/respond-to-interrupt`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ threadId: threadId.value, interruptId: draft.id, approved }),
  })
}

function stopRun() {
  console.info('[storyb00k] stop requested')
  chat.stop()
}

const emit = defineEmits(['edit-in-editor'])

// EDIT on a rendered panel: hand the diagram source + format up to App.vue,
// which flips to the editor view with the source preloaded.
function editPanelSource({ source, format }) {
  console.info('[storyb00k] edit in editor →', format, `${source.length} chars`)
  emit('edit-in-editor', { source, format })
}

function clearThread() {
  console.info('[storyb00k] clearing thread', threadId.value)
  chat.clear()
}

function reloadLast() {
  if (!busy.value) chat.reload()
}

function onTranscriptScroll() {
  const el = transcriptEl.value
  if (!el) return
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 80
}

function formatTokens(u) {
  const total = (u?.promptTokens || 0) + (u?.completionTokens || 0)
  return total ? `${total} tokens` : ''
}
</script>

<template>
  <div class="storyb00k">
    <section class="storyb00k__transcript">
      <header class="storyb00k__header">
        <h2>storyb00k</h2>
        <span class="storyb00k__status" :data-status="status">{{ statusLabel }}</span>
        <span v-if="formatTokens(usage[usage.length - 1])" class="storyb00k__usage">{{ formatTokens(usage[usage.length - 1]) }}</span>
      </header>

      <!-- Project banner: the conceptual unit of work in this session -->
      <div v-if="project" class="storyb00k__project" data-testid="project-banner">
        <div class="storyb00k__project-title">
          <template v-if="editingTitle">
            <input v-model="editedTitle" @keydown.enter="renameProject" @blur="renameProject" data-testid="project-title-input" />
          </template>
          <template v-else>
            <strong>{{ project.title }}</strong>
            <button class="storyb00k__msg-edit" title="Rename project" @click="editedTitle = project.title; editingTitle = true">✏️</button>
          </template>
          <span v-if="project.locked" class="storyb00k__locked" title="Requirements locked in">🔒 locked</span>
        </div>
        <p v-if="project.goal" class="storyb00k__project-goal">{{ project.goal }}</p>
        <p v-if="project.requirements" class="storyb00k__project-reqs"><strong>Locked in:</strong> {{ project.requirements }}</p>
        <p v-if="project.qa.length" class="storyb00k__project-qa" :title="project.qa.map(q => `Q: ${q.question}\nA: ${q.answer}`).join('\n')">
          {{ project.qa.length }} refinement answer{{ project.qa.length === 1 ? '' : 's' }}
        </p>
        <button class="storyb00k__logtoggle" @click="showPromptLog = !showPromptLog">
          {{ showPromptLog ? '▾' : '▸' }} Full prompt &amp; thinking log ({{ promptLog.length + thinkingLog.length }})
        </button>
        <div v-if="showPromptLog" class="storyb00k__log" data-testid="prompt-log">
          <p v-if="!promptLog.length && !thinkingLog.length" class="storyb00k__empty">Nothing logged yet.</p>
          <div v-for="(entry, i) in promptLog" :key="`p${i}`" class="storyb00k__log-entry" :data-role="entry.role">
            <span class="storyb00k__log-role">{{ entry.role }}<template v-if="entry.run"> · {{ entry.run.slice(0, 8) }}</template></span>
            <pre>{{ entry.text }}</pre>
          </div>
          <div v-for="(entry, i) in thinkingLog" :key="`t${i}`" class="storyb00k__log-entry storyb00k__log-entry--thinking">
            <span class="storyb00k__log-role">thinking<template v-if="entry.run"> · {{ entry.run.slice(0, 8) }}</template></span>
            <pre>{{ entry.text }}</pre>
          </div>
        </div>
        <p v-if="projectError" class="storyb00k__error">{{ projectError }}</p>
      </div>
      <p class="storyb00k__lede">Read the live model, assemble evidence panels, and narrate without changing the authoritative model.</p>

      <div ref="transcriptEl" class="storyb00k__messages" @scroll="onTranscriptScroll">
        <p v-if="!items.length" class="storyb00k__empty">
          Ask about this system — e.g. “summarize the physical architecture” or
          “render the deployment as a diagram”. The agent reads the live model;
          changes land only in a disposable draft you approve.
        </p>
        <article v-for="item in items" :key="item.id" class="storyb00k__message" :data-role="item.role">
          <strong>{{ item.role }}:</strong>
          <template v-if="editingMessageId === item.id">
            <div class="storyb00k__edit-box">
              <textarea v-model="editedPrompt" rows="2" data-testid="prompt-editor" />
              <div class="storyb00k__edit-actions">
                <button :disabled="busy" data-testid="regenerate" @click="regeneratePrompt(item)">↻ Regenerate</button>
                <button class="storyb00k__btn-secondary" @click="cancelEditPrompt">Cancel</button>
              </div>
            </div>
          </template>
          <template v-else>
            <template v-for="(part, index) in item.parts" :key="index">
              <span v-if="part.type === 'text'" class="storyb00k__part">{{ part.text }}</span>
              <details v-else-if="part.type === 'reasoning'" class="storyb00k__thinking" :open="part.streaming">
                <summary>💭 thinking{{ part.streaming ? '…' : '' }}</summary>
                <pre class="storyb00k__thinking-text">{{ part.text }}</pre>
              </details>
              <span v-else class="storyb00k__part">[{{ part.type }}]</span>
            </template>
            <button
              v-if="item.role === 'user' && !busy"
              class="storyb00k__msg-edit"
              title="Edit this prompt and regenerate the diagram from the current revision"
              @click="startEditPrompt(item)"
            >✏️</button>
          </template>
        </article>

        <div v-for="tool in toolActivity" :key="tool.id" class="storyb00k__step" data-testid="tool-step">
          <span class="storyb00k__step-name">{{ tool.name }}</span>
          <span v-if="tool.running" class="storyb00k__step-status">running…</span>
          <span v-else-if="tool.failed" class="storyb00k__step-status storyb00k__step-status--error">failed</span>
        </div>

        <div v-for="interrupt in interrupts" :key="interrupt.id" class="storyb00k__interrupt">
          <template v-if="isQuestionInterrupt(interrupt)">
            <p class="storyb00k__question">{{ interrupt.reason || interrupt.message }}</p>
            <div class="storyb00k__choices">
              <label v-for="(option, oi) in optionsFor(interrupt)" :key="oi" class="storyb00k__choice">
                <input type="radio" :name="`q-${interrupt.id}`" :value="option" v-model="questionChoice" />
                {{ option }}
              </label>
            </div>
            <input
              v-if="interrupt.allowFreeText !== false"
              v-model="questionFreeText"
              class="storyb00k__freetext"
              placeholder="…or answer in your own words"
              @keydown.enter="submitAnswer(interrupt.id)"
            />
            <button :disabled="busy || (!questionChoice && !questionFreeText.trim())" data-testid="submit-answer" @click="submitAnswer(interrupt.id)">Answer</button>
          </template>
          <template v-else>
            <p>{{ interrupt.reason || interrupt.message }}</p>
            <button @click="answerInterrupt(interrupt, true)">Approve draft</button>
            <button @click="answerInterrupt(interrupt, false)">Decline</button>
          </template>
        </div>

        <div v-for="draft in drafts.filter(d => d.status === 'pending')" :key="draft.id" class="storyb00k__interrupt">
          <p>Draft proposal: {{ draft.predicate }}={{ JSON.stringify(draft.object) }} on {{ draft.subject }}</p>
          <button @click="decideDraft(draft, true)">Approve</button>
          <button @click="decideDraft(draft, false)">Decline</button>
        </div>
      </div>

      <p v-if="error" class="storyb00k__error" data-testid="run-error">
        {{ error.message }}
        <button class="storyb00k__retry" @click="reloadLast">Retry</button>
      </p>

      <div class="storyb00k__composer">
        <textarea
          v-model="input"
          class="storyb00k__input"
          rows="2"
          placeholder="Ask about the model, request a diagram, or propose a draft change…"
          :disabled="false"
          @keydown.enter.exact.prevent="sendMessage"
        />
        <div class="storyb00k__composer-actions">
          <button v-if="busy" @click="stopRun">Stop</button>
          <button v-else :disabled="!input.trim()" data-testid="send" @click="sendMessage">Send</button>
          <button :disabled="busy" title="Drop everything after the last user message and run it again" @click="reloadLast">Retry</button>
          <button :disabled="busy && !items.length" title="Empty the transcript and reset run state" @click="clearThread">Clear</button>
        </div>
      </div>
    </section>

    <section class="storyb00k__dashboard" aria-label="Agent dashboard">
      <header class="storyb00k__dash-header">
        <h3>Evidence panels</h3>
        <span>{{ panels.length }} panel{{ panels.length === 1 ? '' : 's' }}</span>
        <button v-if="panels.length" class="storyb00k__clear-panels" @click="chat.state = { panels: [], drafts: [] }">Clear panels</button>
      </header>
      <StoryB00kPanel
        v-for="(panel, index) in panels"
        :key="index"
        :panel="panel"
        @edit="editPanelSource"
      />
      <p v-if="!panels.length" class="storyb00k__empty">Rendered diagrams and model data appear here as the agent works.</p>

      <div class="storyb00k__revbar">
        <button class="storyb00k__revtoggle" data-testid="toggle-revisions" @click="showRevisions = !showRevisions">
          {{ showRevisions ? '▾' : '▸' }} Revisions ({{ revisionGraph.nodes.length }})
        </button>
        <span class="storyb00k__rev-active" :title="activeRevision.prompt || activeRevision.label">
          active: {{ activeRevision.label }}
        </span>
        <button :disabled="!activeRevision.source" data-testid="save-chart" title="Serialize the revision graph + active diagram to the local filesystem (jj snapshot)" @click="persistChart(`save ${activeRevision.label}`)">
          💾 Save
        </button>
        <span v-if="saveState" class="storyb00k__savestate">{{ saveState }}</span>
      </div>
      <RevisionFlow
        v-if="showRevisions"
        :graph="revisionGraph"
        @checkout="checkoutRevision"
        @fork="forkRevision"
      />
    </section>
  </div>
</template>

<style scoped>
.storyb00k { display: grid; gap: 1rem; grid-template-columns: minmax(18rem, .8fr) minmax(22rem, 1.2fr); }
.storyb00k__transcript, .storyb00k__dashboard { min-width: 0; display: flex; flex-direction: column; gap: .5rem; }
.storyb00k__header { display: flex; align-items: baseline; gap: .6rem; }
.storyb00k__header h2 { margin: 0; }
.storyb00k__status { font-size: .75rem; padding: .1rem .5rem; border-radius: 999px; background: #e2e8f0; }
.storyb00k__status[data-status='streaming'], .storyb00k__status[data-status='submitted'] { background: #fef3c7; }
.storyb00k__status[data-status='error'] { background: #fee2e2; }
.storyb00k__usage { font-size: .75rem; opacity: .65; }
.storyb00k__lede { margin: 0; font-size: .9rem; opacity: .75; }
.storyb00k__messages { max-height: 55vh; overflow-y: auto; display: flex; flex-direction: column; gap: .4rem; padding-right: .25rem; }
.storyb00k__message { margin: 0; }
.storyb00k__message[data-role='user'] { background: #1b2a52; border-radius: .4rem; padding: .35rem .5rem; }
.storyb00k__step { font-size: .78rem; opacity: .8; display: flex; gap: .5rem; align-items: baseline; }
.storyb00k__step-name { font-family: ui-monospace, monospace; }
.storyb00k__step-status--error { color: #fca5a5; font-weight: 700; }
.storyb00k__interrupt { border-left: 3px solid #b57700; padding-left: .75rem; display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; background: #1c1830; border-radius: .3rem; padding-top: .3rem; padding-bottom: .3rem; }
.storyb00k__interrupt p { margin: 0; flex: 1 1 auto; }
.storyb00k__error { color: #fca5a5; margin: 0; }
.storyb00k__empty { opacity: .65; font-size: .9rem; }
.storyb00k__composer { display: grid; gap: .4rem; }
.storyb00k__input { width: 100%; resize: vertical; font: inherit; border: 1px solid #3b4d7d; border-radius: .45rem; background: #091127; color: #edf5ff; padding: .55rem .65rem; }
.storyb00k__edit-box textarea, .storyb00k__freetext, .storyb00k__project-title input { font: inherit; border: 1px solid #3b4d7d; border-radius: .45rem; background: #091127; color: #edf5ff; padding: .4rem .5rem; }
.storyb00k__project-title input { font-weight: 700; }
.storyb00k__messages { color: #dfe8f7; }
.storyb00k__message[data-role='user'] { background: #1b2a52; }
.storyb00k__status { background: #243465; color: #ced8ee; }
.storyb00k__composer-actions { display: flex; gap: .4rem; }
.storyb00k__clear-panels { font-size: .75rem; }
.storyb00k__dash-header { display: flex; align-items: baseline; gap: .5rem; }
.storyb00k__dash-header h3 { margin: 0; }
.storyb00k__msg-edit { font-size: .7rem; border: 0; background: transparent; cursor: pointer; opacity: .5; }
.storyb00k__msg-edit:hover { opacity: 1; }
.storyb00k__edit-box { display: grid; gap: .3rem; width: 100%; margin-top: .25rem; }
.storyb00k__edit-box textarea { font: inherit; padding: .35rem; }
.storyb00k__edit-actions { display: flex; gap: .4rem; }
.storyb00k__btn-secondary { opacity: .75; }
.storyb00k__revbar { display: flex; align-items: center; gap: .6rem; font-size: .8rem; flex-wrap: wrap; }
.storyb00k__revtoggle { cursor: pointer; }
.storyb00k__rev-active { opacity: .65; max-width: 24ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.storyb00k__savestate { opacity: .7; font-size: .75rem; }
.storyb00k__project { border: 1px solid #2a3966; border-radius: .5rem; padding: .5rem .75rem; display: grid; gap: .25rem; background: #111936; }
.storyb00k__project-title { display: flex; align-items: baseline; gap: .4rem; }
.storyb00k__project-title input { font: inherit; font-weight: 700; }
.storyb00k__locked { font-size: .7rem; background: #14532d; padding: .05rem .4rem; border-radius: 999px; color: #bbf7d0; }
.storyb00k__project-goal { margin: 0; font-size: .85rem; opacity: .8; }
.storyb00k__project-reqs { margin: 0; font-size: .8rem; background: #3a3010; border-radius: .3rem; padding: .25rem .4rem; color: #fde68a; }
.storyb00k__project-qa { margin: 0; font-size: .75rem; opacity: .6; }
.storyb00k__logtoggle { font-size: .75rem; text-align: left; padding: 0; background: none; border: 0; cursor: pointer; opacity: .7; color: #9cc9ff; }
.storyb00k__log { max-height: 14rem; overflow-y: auto; display: grid; gap: .4rem; font-size: .75rem; }
.storyb00k__log-entry { border-left: 2px solid #3b4d7d; padding-left: .5rem; }
.storyb00k__log-entry--thinking { border-left-color: #a855f7; }
.storyb00k__log-role { font-family: ui-monospace, monospace; opacity: .55; font-size: .68rem; text-transform: uppercase; }
.storyb00k__log-entry pre { margin: .1rem 0 0; white-space: pre-wrap; word-break: break-word; font: inherit; }
.storyb00k__thinking { font-size: .78rem; opacity: .9; margin: .15rem 0; }
.storyb00k__thinking summary { cursor: pointer; opacity: .7; }
.storyb00k__thinking-text { margin: .2rem 0 0; white-space: pre-wrap; word-break: break-word; background: #1e1433; border-radius: .3rem; padding: .35rem .5rem; max-height: 10rem; overflow-y: auto; }
.storyb00k__question { font-weight: 600; }
.storyb00k__choices { display: grid; gap: .2rem; width: 100%; }
.storyb00k__choice { display: flex; gap: .4rem; align-items: baseline; cursor: pointer; }
.storyb00k__freetext { flex: 1 1 12rem; }
@media (max-width: 760px) { .storyb00k { grid-template-columns: 1fr; } }
</style>
