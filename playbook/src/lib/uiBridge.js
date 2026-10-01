// Receives UI commands from kr0ki-server (`GET /ui/{session}/events`, SSE) so the planning agent and any
// MCP client can steer this page: filter the gallery, select a type, open a view, highlight a shortlist.
// The server validates every command against the catalog; this side still only acts on a closed set of
// command types and ignores anything else, so a malformed event can never become behaviour.

export const PLANNER_THREAD_PREFIX = 'planner-'

const VIEWS = ['gallery', 'editor', 'agent', 'setup']

// One planner session per browser tab. Its id is also the planner chat's thread id (see StoryB00k `planner`),
// which is how the server knows which tab a `navigate_ui` tool call belongs to.
export function plannerSessionId(storage = globalThis.sessionStorage, rand = () => globalThis.crypto?.randomUUID?.()) {
  const key = 'kr0ki:plannerSession'
  try {
    const existing = storage?.getItem(key)
    if (existing && /^planner-[A-Za-z0-9_-]{1,56}$/.test(existing)) return existing
  } catch { /* storage blocked: fall through to a per-load id */ }
  const raw = rand() || `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 10)}`
  const id = `${PLANNER_THREAD_PREFIX}${raw.replace(/[^A-Za-z0-9_-]/g, '').slice(0, 56)}`
  try { storage?.setItem(key, id) } catch { /* per-load id is fine */ }
  return id
}

// Accept only the documented shapes; return a normalized command or null.
export function parseCommand(data) {
  let env
  try { env = typeof data === 'string' ? JSON.parse(data) : data } catch { return null }
  const c = env?.command
  if (!c || typeof c !== 'object') return null
  const seq = Number.isFinite(env.seq) ? env.seq : null
  switch (c.type) {
    case 'open_view':
      return VIEWS.includes(c.view) ? { seq, type: 'open_view', view: c.view } : null
    case 'filter_gallery':
      return c.use_case == null || typeof c.use_case === 'string'
        ? { seq, type: 'filter_gallery', useCase: c.use_case || 'All' }
        : null
    case 'select_type':
      return typeof c.type_id === 'string' && c.type_id ? { seq, type: 'select_type', typeId: c.type_id } : null
    case 'suggest':
      return Array.isArray(c.type_ids) && c.type_ids.every((t) => typeof t === 'string')
        ? { seq, type: 'suggest', typeIds: c.type_ids, note: typeof c.note === 'string' ? c.note : '' }
        : null
    default:
      return null
  }
}

/**
 * Subscribe to a session. `onCommand(cmd)` gets each parsed command once (replays after a reconnect are
 * dropped by sequence number); `onStatus('connecting'|'open'|'closed')` reports the connection.
 * Returns `{ close }`. `EventSourceImpl` is injectable for tests.
 */
export function connectUiBridge({ baseUrl, sessionId, onCommand, onStatus = () => {}, EventSourceImpl = globalThis.EventSource }) {
  if (!EventSourceImpl || !baseUrl || !sessionId) {
    onStatus('closed')
    return { close() {} }
  }
  const url = `${baseUrl.replace(/\/$/, '')}/ui/${encodeURIComponent(sessionId)}/events`
  let lastSeq = 0
  onStatus('connecting')
  const source = new EventSourceImpl(url)
  source.onopen = () => onStatus('open')
  source.onerror = () => onStatus(source.readyState === 2 ? 'closed' : 'connecting')
  source.addEventListener('ui', (event) => {
    const cmd = parseCommand(event.data)
    if (!cmd) return
    if (cmd.seq != null) {
      if (cmd.seq <= lastSeq) return
      lastSeq = cmd.seq
    }
    onCommand(cmd)
  })
  return { close: () => { source.close(); onStatus('closed') } }
}
