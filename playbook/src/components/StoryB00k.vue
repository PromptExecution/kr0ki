<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { useChat } from '@synoped/ag-ui-vue'
import StoryB00kPanel from './StoryB00kPanel.vue'
import RevisionTimeline from './RevisionTimeline.vue'
import ZoomPan from './ZoomPan.vue'
import CheckpointDialog from './CheckpointDialog.vue'
import { downloadText, renderSvg, sourceFilename } from '../lib/renderSource.js'
import {
  createRevisionGraph, activeNode, addPromptNode, addEditNode,
  checkoutNode, forkFrom, serialize as serializeGraph, nodeForToolCall,
} from '../lib/revisionGraph.js'

const props = defineProps({
  // Gallery → Agent handoff: a sample prompt (names the diagram type) the
  // composer starts with. The user edits/sends it — never auto-sent.
  prefill: { type: String, default: '' },
  lockedType: { type: String, default: '' },
  llmUrl: { type: String, default: '' },
  llmKey: { type: String, default: '' },
  llmModel: { type: String, default: 'gpt-4o' },
  agentUrl: { type: String, default: '' },
  rendererUrl: { type: String, default: '' },
  // Editor → Agent handoff: full diagram data from the editor
  editorHandoff: { type: Object, default: null },
  // Planner mode (gallery side panel): a chat-only column whose thread id doubles as the UI session id, so
  // the planning agent's navigate_ui tool steers this very browser tab. No dashboard, no project banner.
  planner: { type: Boolean, default: false },
  threadId: { type: String, default: '' },
})

// Preserve the browser-visible host so LAN users reach this pod's sidecar instead
// of their own workstation's localhost.
const agentUrl = props.agentUrl || import.meta.env.VITE_STORYB00K_AGENT_URL || `${window.location.protocol}//${window.location.hostname}:8789`

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

const input = ref(props.prefill)
const comparisonPanels = ref([])
const currentOrigin = ref(typeof window !== 'undefined' ? window.location.origin : '')
// Fresh handoffs replace a still-untouched composer; a half-typed draft wins.
watch(() => props.prefill, (next) => {
  if (next && (!input.value.trim() || input.value === props.prefill)) input.value = next
})
// Editor handoff: the "starting point" is derived straight from the prop (no copied state), so it is
// present on first mount, survives sending, and cannot be dismissed -- only collapsed. The user must always
// be able to see where the session started.
const handoffError = ref('')
const startOpen = ref(true)

function validateHandoff(h) {
  if (!h) return 'No handoff data'
  if (!h.source || !h.source.trim()) return 'Missing diagram source'
  if (!h.format) return 'Missing diagram format'
  return null
}

const startingPoint = computed(() => (validateHandoff(props.editorHandoff) ? null : props.editorHandoff))
const handoffProblem = computed(() => (props.editorHandoff ? validateHandoff(props.editorHandoff) : null))
watch(() => props.editorHandoff, (handoff) => {
  handoffError.value = ''
  startOpen.value = true
  startDraft.value = handoff?.source || ''
  startSvg.value = ''
  startRenderState.value = ''
  // A new handoff arriving while this panel is alive becomes a new revision on top of what is there (kept alive across tabs).
  if (handoff?.source?.trim() && handoff.source !== activeNode(revisionGraph).source) {
    const n = addEditNode(revisionGraph, { source: handoff.source, format: handoff.format, route: handoff.route || null, label: handoff.title ? `From Code Editor: ${handoff.title}` : 'From Code Editor' })
    n.detectedType = handoff.detectedType
    n.output = handoff.output
    n.title = handoff.title
  }
  if (handoff) console.info('[storyb00k] handoff received:', {
    format: handoff.format, detectedType: handoff.detectedType,
    title: handoff.title, sourceLen: handoff.source?.length ?? 0,
  })
})
// The starting point is editable and renderable in place: edit the code, Render to see it, and apply it as a new revision (the
// agent's next request then starts from your edit). Resets when a new handoff arrives.
const startDraft = ref(props.editorHandoff?.source || '')
const startSvg = ref('')
const startRenderState = ref('')
async function renderStart() {
  const h = startingPoint.value
  if (!h) return
  startRenderState.value = 'rendering…'
  try {
    startSvg.value = await renderSvg({ rendererUrl: props.rendererUrl, format: h.format, route: h.route || null, source: startDraft.value })
    startRenderState.value = ''
  } catch (err) {
    startSvg.value = ''
    startRenderState.value = `Render failed: ${err.message}`
  }
}
function applyStartEdit() {
  const h = startingPoint.value
  if (!h || !startDraft.value.trim() || startDraft.value === activeRevision.value.source) return
  addEditNode(revisionGraph, { source: startDraft.value, format: h.format, route: h.route || null, label: 'Edited starting point' })
  saveState.value = 'applied your edit as a new revision'
}
const autoScroll = ref(true)
const transcriptEl = ref(null)
const chat = useChat({
  url: `${agentUrl}/run`,
  ...(props.threadId ? { threadId: props.threadId } : {}),
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
const panels = computed(() => [...(chat.state.value?.panels || []), ...comparisonPanels.value])
const drafts = computed(() => chat.state.value?.drafts || [])
const busy = computed(() => status.value === 'submitted' || status.value === 'streaming')
const agentConnectionStatus = ref('unknown') // 'unknown', 'checking', 'connected', 'failed'
const agentConnectionError = ref('')

// Test agent connection on mount and provide detailed diagnostics
async function testAgentConnection() {
  agentConnectionStatus.value = 'checking'
  agentConnectionError.value = ''
  
  const currentOrigin = window.location.origin
  
  try {
    const controller = new AbortController()
    const timeoutId = setTimeout(() => controller.abort(), 5000)
    
    const response = await fetch(`${agentUrl}/health`, {
      method: 'GET',
      signal: controller.signal,
    })
    clearTimeout(timeoutId)
    
    if (response.ok) {
      agentConnectionStatus.value = 'connected'
      console.info('[storyb00k] ✓ Agent server connected:', agentUrl)
    } else {
      agentConnectionStatus.value = 'failed'
      agentConnectionError.value = `Agent server returned HTTP ${response.status}`
      console.error('[storyb00k] ✗ Agent server error:', agentUrl, 'HTTP', response.status)
    }
  } catch (err) {
    agentConnectionStatus.value = 'failed'
    
    if (err.name === 'AbortError') {
      agentConnectionError.value = `Connection timeout (5s) - agent server at ${agentUrl} is not responding`
    } else if (err.message.includes('Failed to fetch')) {
      // This could be a network error OR a CORS error
      // Try to detect CORS by checking if we can reach the server at all
      try {
        // Try a no-cors request to see if the server is reachable
        const testResponse = await fetch(`${agentUrl}/health`, {
          method: 'GET',
          mode: 'no-cors',
        })
        
        // If we got here with no-cors, the server is reachable but CORS is blocking
        agentConnectionError.value = `CORS error: Agent server at ${agentUrl} is reachable, but does not allow requests from ${currentOrigin}. ` +
          `Update KR0KI_STORYB00K_ALLOWED_ORIGINS to include ${currentOrigin}`
        console.error('[storyb00k] ✗ CORS error detected:', {
          agentUrl,
          currentOrigin,
          suggestion: `Add ${currentOrigin} to KR0KI_STORYB00K_ALLOWED_ORIGINS`
        })
      } catch (testErr) {
        // Server is not reachable at all
        agentConnectionError.value = `Cannot reach agent server at ${agentUrl} - is it running? ` +
          `Check that the server is started and the URL is correct.`
        console.error('[storyb00k] ✗ Network error:', agentUrl, testErr.message)
      }
    } else {
      agentConnectionError.value = `Connection failed: ${err.message}`
      console.error('[storyb00k] ✗ Agent connection failed:', agentUrl, err.message)
    }
  }
}

// Test connection on mount
testAgentConnection()
// What each tool call did, in the order it happened: arguments, outcome, and (for renders) the revision it produced.
// A call "failed" when the tracker says so OR its result text says the tool failed / was held back (the agent reports
// those as ordinary results, so the tracker alone would call them successes).
function toolOutcome(t) {
  if (t.state === 'output-error' || t.state === 'output-denied' || t.error) return 'failed'
  if (t.state === 'input-streaming' || t.state === 'input-available' || t.state === 'approval-requested') return 'running'
  if (/^tool \S+ failed:|was NOT run yet/.test(String(t.output || ''))) return 'failed'
  return 'ok'
}
function prettyArgs(raw) {
  try { return JSON.stringify(JSON.parse(raw || '{}'), null, 2) } catch { return String(raw || '') }
}
const toolActivity = computed(() => Array.from(toolCallTrackers.value?.values() || []).map((t) => {
  const outcome = toolOutcome(t)
  const rev = nodeForToolCall(revisionGraph, t.toolCallId)
  return {
    id: t.toolCallId,
    name: t.toolName,
    running: outcome === 'running',
    failed: outcome === 'failed',
    ok: outcome === 'ok',
    args: prettyArgs(t.args),
    output: t.output || t.error || '',
    revisionId: rev?.id || '',
  }
}))
const toolOutcomes = computed(() => Object.fromEntries(toolActivity.value.map((t) => [t.id, { name: t.name, ok: t.ok }])))
const openTool = ref('')
function showToolRevision(tool) {
  if (!tool.revisionId) return
  showRevisions.value = true
  revisionFocus.value = tool.revisionId
}
const revisionFocus = ref('')
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

// ---- Fast-track (Plan 005 UX): after 2 answered questions the user can
// skip further questions and authorize best-judgement rendering immediately.
const canFastTrack = computed(() => (project.value?.qa?.length || 0) >= 2 && !project.value?.fastTrack)
const fastTrackArmed = computed(() => !!project.value?.fastTrack)

async function diagramNow() {
  if (busy.value) return
  try {
    const res = await fetch(`${agentUrl}/projects/fasttrack`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ threadId: threadId.value, enabled: true }),
    })
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    await loadProject()
    await chat.send('Diagram now — use your best judgement from what you know so far.')
    loadProject()
  } catch (err) {
    console.error('[storyb00k] diagram-now failed:', err?.message ?? err)
    projectError.value = `diagram-now failed: ${err.message}`
  }
}

const statusLabel = computed(() => ({
  ready: 'Ready',
  submitted: 'Thinking…',
  streaming: 'Working…',
  error: 'Error',
}[status.value] || status.value))

// ---- Revision graph: every prompt/render is a node; time travel + forks ----
// Initialize with editor handoff source if available (Code Editor → Agent flow)
const initialSource = props.editorHandoff?.source || ''
const initialFormat = props.editorHandoff?.format || 'd2'
const initialLabel = props.editorHandoff ? 'Initial diagram from Code Editor' : 'Session start'
const revisionGraph = reactive(createRevisionGraph({ 
  source: initialSource, 
  format: initialFormat, 
  label: initialLabel 
}))
const showRevisions = ref(false)
const editingMessageId = ref(null)
const editedPrompt = ref('')
const activeRevision = computed(() => activeNode(revisionGraph))
const saveState = ref('')

// If editor handoff provided, mark the root node with the handoff metadata
if (props.editorHandoff) {
  const rootNode = revisionGraph.nodes[0]
  if (rootNode) {
    rootNode.detectedType = props.editorHandoff.detectedType
    rootNode.output = props.editorHandoff.output
    rootNode.title = props.editorHandoff.title
    console.info('[storyb00k] initialized revision graph with Code Editor handoff:', {
      source: initialSource.substring(0, 50) + '...',
      format: initialFormat,
      detectedType: props.editorHandoff.detectedType
    })
  }
}

// Watch panel renders: when the agent produces a new diagram, record it as a
// prompt node (the prompt that produced it) on top of the active revision.
// Every render becomes a revision, once (keyed by the tool call that made it; by source when the panel has no id), with
// its picture so the timeline can show it later without re-rendering.
function lastUserText() {
  const u = [...items.value].reverse().find((i) => i.role === 'user')
  return (u?.parts || []).map((p) => (p.type === 'text' ? p.text : '')).join('').trim()
}
watch(panels, (list) => {
  for (const p of list) {
    if (p.kind !== 'render' || !p.source?.text) continue
    if (p.toolCallId ? nodeForToolCall(revisionGraph, p.toolCallId) : revisionGraph.nodes.some((n) => n.source === p.source.text && n.kind !== 'root')) continue
    addPromptNode(revisionGraph, {
      prompt: lastUserText() || p.toolName || 'agent render',
      source: p.source.text,
      format: p.source.format || 'd2',
      route: p.source.route || null,
      notes: '',
      rendered: p.imageDataUrl ? { imageDataUrl: p.imageDataUrl } : p.content ? { svg: p.content } : null,
      toolCallId: p.toolCallId || null,
      toolName: p.toolName || null,
    })
    saveState.value = ''
  }
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

// Restoring only changes which revision is current (the agent's next request starts from it); the evidence panels and every
// other revision are left alone, so nothing is lost by going back.
function checkoutRevision(nodeId) {
  checkoutNode(revisionGraph, nodeId)
  console.info('[storyb00k] restored revision →', nodeId)
  saveState.value = `restored ${activeNode(revisionGraph).label}`
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
  
  // Check agent connection before sending
  if (agentConnectionStatus.value !== 'connected') {
    console.warn('[storyb00k] Attempting to send message but agent connection status is:', agentConnectionStatus.value)
    // Try to reconnect
    await testAgentConnection()
    if (agentConnectionStatus.value !== 'connected') {
      console.error('[storyb00k] Cannot send message - agent server not available at', agentUrl)
      return
    }
  }
  
  // The agent starts from the CURRENT revision (the editor handoff at first; later the agent's last render, an edit, or a
  // revision the user restored). The server consumes this once per run, so it is sent with every message.
  const cur = activeRevision.value
  if (cur?.source?.trim() && cur.format) {
    try {
      const contextRes = await fetch(`${agentUrl}/projects/set-diagram-context`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          threadId: threadId.value,
          source: cur.source,
          format: cur.format,
          detectedType: cur.detectedType || undefined,
          output: cur.output || undefined,
          imageData: cur.kind === 'root' ? props.editorHandoff?.imageData : undefined,
          title: cur.title || cur.label,
        }),
      })
      if (!contextRes.ok) {
        const errBody = await contextRes.text().catch(() => '')
        console.warn('[storyb00k] Failed to set diagram context:', contextRes.status, errBody)
        handoffError.value = `Could not give the agent the current diagram: HTTP ${contextRes.status}`
      } else {
        handoffError.value = ''
      }
    } catch (err) {
      console.error('[storyb00k] Error setting diagram context:', err)
      handoffError.value = `Could not give the agent the current diagram: ${err.message}`
    }
  }

  input.value = ''
  console.info('[storyb00k] send →', text, '| thread:', threadId.value, '| agent:', agentUrl)
  try {
    if (props.lockedType) {
      const lock = await fetch(`${agentUrl}/projects/lock-type`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ threadId: threadId.value, typeId: props.lockedType }),
      })
      if (!lock.ok) throw new Error(`type lock failed: HTTP ${lock.status}`)
    }
    const result = await chat.send(text)
    console.info('[storyb00k] run finished | new messages:', result?.newMessages?.length ?? 0,
      '| usage:', usage.value.at(-1) ?? 'none reported')
  } catch (err) {
    const errorMsg = err?.message ?? String(err)
    console.error('[storyb00k] run failed:', errorMsg, '| agent:', agentUrl)
    // Enhance error with connection details
    if (errorMsg.includes('Failed to fetch')) {
      console.error(`[storyb00k] Network error - cannot reach agent at ${agentUrl}`)
      console.error('[storyb00k] LLM config:', {
        url: props.llmUrl,
        model: props.llmModel,
        hasKey: !!props.llmKey
      })
    }
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
  // ask-* = discovery/refinement questions; rec-* = type recommendations.
  // Both render the multiple-choice answer UI (Approve/Decline is for
  // draft proposals only — posting `approved` on a question yields 400).
  const id = interrupt.id
  return typeof id === 'string' && (id.startsWith('ask-') || id.startsWith('rec-'))
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
    const response = await res.json()
    if (response.comparisonPanels?.length) {
      comparisonPanels.value = [...comparisonPanels.value, ...response.comparisonPanels]
      questionChoice.value = ''
      questionFreeText.value = ''
      chat.respondToInterrupt(interrupt.id, { answer })
      await loadProject()
      return
    }
    questionChoice.value = ''
    questionFreeText.value = ''
    // Record the response with the client, then RESUME the interrupted run —
    // chat.send() would start a new run, which the agent rejects while
    // interrupts are pending ("Thread has N pending interrupt(s) not
    // addressed by resume"). resume() carries the recorded response payload.
    chat.respondToInterrupt(interrupt.id, { answer })
    await chat.resume()
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

// "Clear chat" empties the transcript the agent is sent. The project record on the server (goal, answers), the evidence panels
// and every revision are kept, so nothing that was made is lost.
function clearThread() {
  console.info('[storyb00k] clearing thread', threadId.value)
  chat.clear()
}

// "Hide panels" only empties the evidence column. Revisions (with their pictures) are untouched and still listed below.
function clearPanels() {
  chat.agent.setState({ ...(chat.state.value || {}), panels: [] })
  chat.state.value = chat.agent.state
  comparisonPanels.value = []
}

// What the agent receives on the next request, shown so there is no hidden state to guess at.
const showContext = ref(false)
const contextSummary = computed(() => ({
  thread: threadId.value,
  messages: items.value.length,
  diagram: activeRevision.value.source ? `${activeRevision.value.format}, ${activeRevision.value.source.split('\n').length} lines (revision "${activeRevision.value.label}")` : 'none',
  answers: project.value?.qa?.length || 0,
  lockedType: project.value?.lockedType || props.lockedType || '',
  goal: project.value?.goal || '',
}))

function downloadCurrent() {
  const r = activeRevision.value
  if (r.source) downloadText(sourceFilename(r.format, 'diagram'), r.source)
}
function downloadStart() {
  const h = startingPoint.value
  if (h) downloadText(sourceFilename(h.format, 'starting-point'), startDraft.value)
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
  <div class="storyb00k" :class="{ 'storyb00k--planner': planner }" :data-planner="planner || undefined">
    <section class="storyb00k__transcript">
      <header class="storyb00k__header">
        <h2>{{ planner ? 'planner' : 'storyb00k' }}</h2>
        <span class="storyb00k__status" :data-status="status">{{ statusLabel }}</span>
        <span v-if="formatTokens(usage[usage.length - 1])" class="storyb00k__usage">{{ formatTokens(usage[usage.length - 1]) }}</span>
        <span class="storyb00k__agent-status" :data-status="agentConnectionStatus" :title="agentConnectionError || `Agent: ${agentUrl}`">
          {{ agentConnectionStatus === 'connected' ? '✓ agent' : agentConnectionStatus === 'checking' ? '… agent' : '✗ agent' }}
        </span>
        <button v-if="agentConnectionStatus === 'failed'" class="storyb00k__retry-connection" @click="testAgentConnection" title="Retry agent connection">
          ↻
        </button>
      </header>

      <!-- Project banner: the conceptual unit of work in this session -->
      <div v-if="project && !planner" class="storyb00k__project" data-testid="project-banner">
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
        <div v-if="canFastTrack" class="storyb00k__fasttrack">
          <span class="storyb00k__fasttrack-hint">Enough questions?</span>
          <button data-testid="diagram-now" :disabled="busy" @click="diagramNow">⚡ Diagram now</button>
        </div>
        <p v-else-if="fastTrackArmed" class="storyb00k__project-qa" title="The agent will use best judgement without asking further questions">⚡ Fast-tracked — best judgement only</p>
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
      <!-- Starting point: the code and diagram transferred from the editor; always visible, collapsible only -->
      <section v-if="startingPoint" class="storyb00k__start" data-testid="starting-point">
        <button type="button" class="storyb00k__start-head" :aria-expanded="startOpen" @click="startOpen = !startOpen">
          <span>{{ startOpen ? '▾' : '▸' }} 📋 Starting point — from Code Editor</span>
          <span class="storyb00k__handoff-meta">{{ startingPoint.title }}<template v-if="startingPoint.title"> · </template>{{ startingPoint.detectedType && startingPoint.detectedType !== 'unknown' ? startingPoint.detectedType + ' · ' : '' }}{{ startingPoint.format }}</span>
        </button>
        <div v-show="startOpen" class="storyb00k__start-body">
          <ZoomPan v-if="startSvg" :content-key="startSvg.length" height="18rem">
            <div class="storyb00k__start-svg" data-testid="starting-render" v-html="startSvg"></div>
          </ZoomPan>
          <img v-else-if="startingPoint.imageData" :src="startingPoint.imageData" alt="Diagram transferred from the Code Editor" data-testid="starting-image" />
          <p v-else class="storyb00k__empty">No picture yet: press Render.</p>
          <textarea v-model="startDraft" class="storyb00k__start-source" rows="8" spellcheck="false" aria-label="Starting diagram code" data-testid="starting-source"></textarea>
          <div class="storyb00k__start-actions">
            <button type="button" data-testid="start-render" @click="renderStart">▶ Render</button>
            <button type="button" data-testid="start-apply" :disabled="!startDraft.trim() || startDraft === activeRevision.source" title="Make your edit the current revision; the agent's next request starts from it" @click="applyStartEdit">Use my edit</button>
            <button type="button" data-testid="start-editor" title="Open in the Code Editor (renders there, with language-server help)" @click="emit('edit-in-editor', { source: startDraft, format: startingPoint.format, route: startingPoint.route })">✏️ Open in editor</button>
            <button type="button" data-testid="start-download" @click="downloadStart">⬇ Code</button>
            <span v-if="startRenderState" class="storyb00k__savestate" data-testid="start-render-state">{{ startRenderState }}</span>
          </div>
        </div>
      </section>
      <p v-if="planner" class="storyb00k__lede">Tell me what you want to show. I'll narrow the gallery and suggest the best-fit diagram type.</p>
      <p v-else class="storyb00k__lede">Read the live model, assemble evidence panels, and narrate without changing the authoritative model.</p>

      <div ref="transcriptEl" class="storyb00k__messages" @scroll="onTranscriptScroll">
        <p v-if="!items.length && !planner" class="storyb00k__empty">
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

        <details v-for="tool in toolActivity" :key="tool.id" class="storyb00k__step" data-testid="tool-step" :open="openTool === tool.id" @toggle="openTool = $event.target.open ? tool.id : (openTool === tool.id ? '' : openTool)">
          <summary>
            <span class="storyb00k__step-name">{{ tool.name }}</span>
            <span v-if="tool.running" class="storyb00k__step-status">running…</span>
            <span v-else-if="tool.failed" class="storyb00k__step-status storyb00k__step-status--error" data-testid="tool-failed">✗ failed</span>
            <span v-else class="storyb00k__step-status storyb00k__step-status--ok" data-testid="tool-ok">✓ ok</span>
            <button v-if="tool.revisionId" type="button" class="storyb00k__step-rev" data-testid="tool-revision" title="Show the diagram this call produced, in the revision timeline" @click.prevent="showToolRevision(tool)">⏱ revision</button>
          </summary>
          <p class="storyb00k__step-label">Arguments</p>
          <pre class="storyb00k__step-pre">{{ tool.args }}</pre>
          <p class="storyb00k__step-label">Result</p>
          <pre class="storyb00k__step-pre">{{ tool.output ? tool.output.slice(0, 4000) : '(none yet)' }}</pre>
        </details>

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
        <strong>Error:</strong> {{ error.message }}
        <span v-if="agentConnectionStatus === 'failed'" class="storyb00k__error-detail">
          <br>Agent server: <code>{{ agentUrl }}</code>
          <br>Browser origin: <code>{{ currentOrigin }}</code>
          <br v-if="agentConnectionError">{{ agentConnectionError }}
          <span v-if="agentConnectionError && agentConnectionError.includes('CORS')" class="storyb00k__error-hint">
            <strong>Fix:</strong> Restart the agent server with:<br>
            <code>export KR0KI_STORYB00K_ALLOWED_ORIGINS="{{ currentOrigin }},http://127.0.0.1:8787"</code>
          </span>
        </span>
        <span v-else class="storyb00k__error-detail">
          <br>Agent: <code>{{ agentUrl }}</code>
          <br>LLM: <code>{{ llmUrl || 'not configured' }}</code>
          <br>Model: {{ llmModel || 'not configured' }}
        </span>
        <button class="storyb00k__retry" @click="reloadLast">Retry</button>
      </p>

      <div class="storyb00k__composer">
        <p v-if="handoffError || handoffProblem" class="storyb00k__handoff-error" data-testid="handoff-error">{{ handoffError || handoffProblem }}</p>
        <textarea
          v-model="input"
          class="storyb00k__input"
          rows="2"
          :placeholder="planner ? 'e.g. I need to show how our services call each other…' : 'Ask about the model, request a diagram, or propose a draft change…'"
          :disabled="false"
          @keydown.enter.exact.prevent="sendMessage"
        />
        <div class="storyb00k__composer-actions">
          <button v-if="busy" @click="stopRun">Stop</button>
          <button v-else :disabled="!input.trim()" data-testid="send" @click="sendMessage">Send</button>
          <button :disabled="busy" title="Drop everything after the last user message and run it again" @click="reloadLast">Retry</button>
          <button :disabled="busy && !items.length" title="Empty the chat transcript. Your project record, the evidence panels and all revisions are kept." @click="clearThread">Clear chat</button>
          <button type="button" data-testid="toggle-context" title="What the agent receives with your next message" @click="showContext = !showContext">{{ showContext ? '▾' : '▸' }} Context</button>
        </div>
      </div>
      <dl v-if="showContext" class="storyb00k__context" data-testid="context-info">
        <dt>Sent with every message</dt>
        <dd>this chat's {{ contextSummary.messages }} message{{ contextSummary.messages === 1 ? '' : 's' }} (nothing else is remembered from earlier runs), plus the current diagram: {{ contextSummary.diagram }}.</dd>
        <dt>Kept on the server for this thread ({{ contextSummary.thread.slice(0, 8) }}…)</dt>
        <dd>your goal{{ contextSummary.goal ? ` ("${contextSummary.goal.slice(0, 60)}")` : '' }}, {{ contextSummary.answers }} answered question{{ contextSummary.answers === 1 ? '' : 's' }}, the diagram type lock{{ contextSummary.lockedType ? ` (${contextSummary.lockedType})` : ' (none)' }}, and the model-change drafts awaiting approval.</dd>
        <dt>Also in the agent's instructions</dt>
        <dd>its fixed role, the syntax skill for the diagram type (loaded when the type is locked or a render needs it), and any MBSE skill it chooses to load: that is a visible tool call above, with its arguments.</dd>
      </dl>
    </section>

    <section v-if="!planner" class="storyb00k__dashboard" aria-label="Agent dashboard">
      <header class="storyb00k__dash-header">
        <h3>Evidence panels</h3>
        <span>{{ panels.length }} panel{{ panels.length === 1 ? '' : 's' }}</span>
        <span v-if="busy" class="storyb00k__spinner" data-testid="panel-spinner" role="status" aria-label="Diagram generating" title="Generating diagram…"></span>
        <button v-if="panels.length" class="storyb00k__clear-panels" data-testid="hide-panels" title="Hide these evidence panels. Nothing is deleted: every render is still in the revision timeline below." @click="clearPanels">Hide panels</button>
      </header>
      <p v-if="busy" class="storyb00k__generating" data-testid="generating-note">
        <span class="storyb00k__spinner"></span> Generating diagram<span class="storyb00k__dots">…</span>
      </p>
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
          current: {{ activeRevision.label }}
        </span>
        <button :disabled="!activeRevision.source" data-testid="download-current" title="Download the current revision's code" @click="downloadCurrent">⬇ Code</button>
        <button :disabled="!activeRevision.source" data-testid="save-chart" title="Save the revision history and current diagram on the agent server (versioned with jj when available)" @click="persistChart(`save ${activeRevision.label}`)">
          💾 Save
        </button>
        <span v-if="saveState" class="storyb00k__savestate" data-testid="save-state">{{ saveState }}</span>
      </div>
      <CheckpointDialog :source="activeRevision.source" :format="activeRevision.format" :title="activeRevision.title || activeRevision.label" @saved="saveState = `checkpoint saved to project`" />
      <RevisionTimeline
        v-if="showRevisions"
        :graph="revisionGraph"
        :tool-calls="toolOutcomes"
        :focus-id="revisionFocus"
        :renderer-url="rendererUrl"
        @restore="checkoutRevision"
        @fork="forkRevision"
      />
    </section>
  </div>
</template>

<style scoped>
.storyb00k { display: grid; gap: 1rem; grid-template-columns: minmax(18rem, .8fr) minmax(22rem, 1.2fr); }
.storyb00k--planner { grid-template-columns: 1fr; }
.storyb00k__transcript, .storyb00k__dashboard { min-width: 0; display: flex; flex-direction: column; gap: .5rem; }
.storyb00k__header { display: flex; align-items: baseline; gap: .6rem; }
.storyb00k__header h2 { margin: 0; }
.storyb00k__status { font-size: .75rem; padding: .1rem .5rem; border-radius: 999px; background: #e2e8f0; }
.storyb00k__status[data-status='streaming'], .storyb00k__status[data-status='submitted'] { background: #fef3c7; }
.storyb00k__status[data-status='error'] { background: #fee2e2; }
.storyb00k__usage { font-size: .75rem; opacity: .65; }
.storyb00k__agent-status { font-size: .7rem; padding: .1rem .4rem; border-radius: 999px; background: #e2e8f0; margin-left: auto; }
.storyb00k__agent-status[data-status='connected'] { background: #10b981; color: #fff; }
.storyb00k__agent-status[data-status='checking'] { background: #fbbf24; color: #000; }
.storyb00k__agent-status[data-status='failed'] { background: #ef4444; color: #fff; cursor: help; }
.storyb00k__retry-connection { background: none; border: 1px solid #ef4444; color: #ef4444; border-radius: .3rem; padding: .1rem .4rem; font-size: .75rem; cursor: pointer; margin-left: .3rem; }
.storyb00k__retry-connection:hover { background: #ef4444; color: #fff; }
.storyb00k__lede { margin: 0; font-size: .9rem; opacity: .75; }
.storyb00k__messages { max-height: 55vh; overflow-y: auto; display: flex; flex-direction: column; gap: .4rem; padding-right: .25rem; }
.storyb00k__message { margin: 0; }
.storyb00k__message[data-role='user'] { background: #1b2a52; border-radius: .4rem; padding: .35rem .5rem; }
.storyb00k__step { font-size: .78rem; opacity: .8; display: flex; gap: .5rem; align-items: baseline; }
.storyb00k__step-name { font-family: ui-monospace, monospace; }
.storyb00k__step-status--error { color: #fca5a5; font-weight: 700; }
.storyb00k__interrupt { border-left: 3px solid #b57700; padding-left: .75rem; display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; background: #1c1830; border-radius: .3rem; padding-top: .3rem; padding-bottom: .3rem; }
.storyb00k__interrupt p { margin: 0; flex: 1 1 auto; }
.storyb00k__error { color: #fca5a5; margin: 0; padding: .5rem; background: #450a0a; border-radius: .3rem; font-size: .85rem; }
.storyb00k__error code { background: #1e293b; padding: .1rem .3rem; border-radius: .2rem; font-size: .8rem; }
.storyb00k__error-detail { display: block; margin-top: .3rem; font-size: .75rem; opacity: .85; }
.storyb00k__error-hint { display: block; margin-top: .5rem; padding: .4rem; background: #1e293b; border-radius: .3rem; font-size: .75rem; opacity: 1; }
.storyb00k__error-hint code { display: block; margin-top: .3rem; padding: .3rem; background: #0f172a; word-break: break-all; }
.storyb00k__empty { opacity: .65; font-size: .9rem; }
.storyb00k__composer { display: grid; gap: .4rem; }
.storyb00k__start { border: 1px solid #38bdf8; border-radius: .45rem; background: #0c2d48; color: #bae6fd; }
.storyb00k__start-head { all: unset; box-sizing: border-box; width: 100%; display: flex; flex-wrap: wrap; gap: .5rem; justify-content: space-between; padding: .4rem .6rem; font-weight: 700; font-size: .85rem; cursor: pointer; }
.storyb00k__start-body { display: grid; gap: .5rem; padding: 0 .6rem .6rem; }
.storyb00k__start-body img { max-width: 100%; max-height: 260px; object-fit: contain; background: #fff; border-radius: .3rem; justify-self: start; }
.storyb00k__start-body pre { margin: 0; max-height: 220px; overflow: auto; padding: .5rem; background: #091127; border-radius: .3rem; font-size: .75rem; white-space: pre-wrap; }
.storyb00k__handoff-meta { opacity: .7; font-size: .75rem; font-weight: 400; }
.storyb00k__handoff-error { color: #fca5a5; background: #450a0a; padding: .3rem .5rem; border-radius: .3rem; font-size: .8rem; margin: 0; }
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
.storyb00k__spinner {
  width: 1rem; height: 1rem; flex: 0 0 auto;
  border: 2px solid #3b4d7d; border-top-color: #38bdf8;
  border-radius: 50%; display: inline-block;
  animation: storyb00k-spin .8s linear infinite;
}
@keyframes storyb00k-spin { to { transform: rotate(360deg); } }
.storyb00k__generating { margin: 0; display: flex; align-items: center; gap: .5rem; color: #9cc9ff; font-size: .85rem; }
.storyb00k__dots { animation: storyb00k-pulse 1.2s ease-in-out infinite; }
@keyframes storyb00k-pulse { 0%, 100% { opacity: .3; } 50% { opacity: 1; } }
.storyb00k__fasttrack { display: flex; align-items: center; gap: .5rem; }
.storyb00k__fasttrack-hint { font-size: .75rem; opacity: .6; }
.storyb00k__fasttrack button { border: 0; border-radius: .45rem; background: #38bdf8; color: #052235; padding: .35rem .7rem; font-weight: 800; cursor: pointer; font-size: .8rem; }
.storyb00k__fasttrack button:hover { filter: brightness(1.1); }
.storyb00k__fasttrack button:disabled { opacity: .5; cursor: wait; }
@media (max-width: 760px) { .storyb00k { grid-template-columns: 1fr; } }

.storyb00k__start-source { width: 100%; box-sizing: border-box; font-family: ui-monospace, monospace; font-size: .8rem; }
.storyb00k__start-actions { display: flex; flex-wrap: wrap; gap: .4rem; align-items: center; }
.storyb00k__start-svg { max-width: 100%; overflow: auto; }
.storyb00k__start-svg :deep(svg) { max-width: 100%; height: auto; }
.storyb00k__step summary { display: flex; gap: .5rem; align-items: center; cursor: pointer; }
.storyb00k__step-status--ok { color: #15803d; }
.storyb00k__step-rev { font-size: .72rem; padding: .05rem .4rem; cursor: pointer; margin-left: auto; }
.storyb00k__step-label { margin: .4rem 0 .1rem; font-size: .7rem; text-transform: uppercase; opacity: .65; }
.storyb00k__step-pre { margin: 0; padding: .4rem; background: rgba(100, 116, 139, .12); border-radius: 4px; white-space: pre-wrap; overflow-x: auto; max-height: 12rem; overflow-y: auto; font-size: .76rem; }
.storyb00k__context { margin: .4rem 0 0; font-size: .8rem; padding: .5rem .7rem; border: 1px solid #c8ced8; border-radius: 6px; }
.storyb00k__context dt { font-weight: 700; margin-top: .3rem; }
.storyb00k__context dd { margin: 0; opacity: .85; }
</style>
