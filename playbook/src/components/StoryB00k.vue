<script setup>
import { computed, ref } from 'vue'
import { useChat } from '@synoped/ag-ui-vue'
import StoryB00kPanel from './StoryB00kPanel.vue'

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
const statusLabel = computed(() => ({
  ready: 'Ready',
  submitted: 'Thinking…',
  streaming: 'Working…',
  error: 'Error',
}[status.value] || status.value))

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
      <p class="storyb00k__lede">Read the live model, assemble evidence panels, and narrate without changing the authoritative model.</p>

      <div ref="transcriptEl" class="storyb00k__messages" @scroll="onTranscriptScroll">
        <p v-if="!items.length" class="storyb00k__empty">
          Ask about this system — e.g. “summarize the physical architecture” or
          “render the deployment as a diagram”. The agent reads the live model;
          changes land only in a disposable draft you approve.
        </p>
        <article v-for="item in items" :key="item.id" class="storyb00k__message" :data-role="item.role">
          <strong>{{ item.role }}:</strong>
          <span v-for="(part, index) in item.parts" :key="index" class="storyb00k__part">
            {{ part.type === 'text' ? part.text : `[${part.type}]` }}
          </span>
        </article>

        <div v-for="tool in toolActivity" :key="tool.id" class="storyb00k__step" data-testid="tool-step">
          <span class="storyb00k__step-name">{{ tool.name }}</span>
          <span v-if="tool.running" class="storyb00k__step-status">running…</span>
          <span v-else-if="tool.failed" class="storyb00k__step-status storyb00k__step-status--error">failed</span>
        </div>

        <div v-for="interrupt in interrupts" :key="interrupt.id" class="storyb00k__interrupt">
          <p>{{ interrupt.reason || interrupt.message }}</p>
          <button @click="answerInterrupt(interrupt, true)">Approve draft</button>
          <button @click="answerInterrupt(interrupt, false)">Decline</button>
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
.storyb00k__message[data-role='user'] { background: #eef2ff; border-radius: .4rem; padding: .35rem .5rem; }
.storyb00k__step { font-size: .78rem; opacity: .8; display: flex; gap: .5rem; align-items: baseline; }
.storyb00k__step-name { font-family: ui-monospace, monospace; }
.storyb00k__step-status--error { color: #b91c1c; font-weight: 700; }
.storyb00k__interrupt { border-left: 3px solid #b57700; padding-left: .75rem; display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; }
.storyb00k__interrupt p { margin: 0; flex: 1 1 auto; }
.storyb00k__error { color: #b91c1c; margin: 0; }
.storyb00k__empty { opacity: .65; font-size: .9rem; }
.storyb00k__composer { display: grid; gap: .4rem; }
.storyb00k__input { width: 100%; resize: vertical; font: inherit; }
.storyb00k__composer-actions { display: flex; gap: .4rem; }
.storyb00k__clear-panels { font-size: .75rem; }
.storyb00k__dash-header { display: flex; align-items: baseline; gap: .5rem; }
.storyb00k__dash-header h3 { margin: 0; }
@media (max-width: 760px) { .storyb00k { grid-template-columns: 1fr; } }
</style>
