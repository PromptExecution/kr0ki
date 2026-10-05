// Client and presentation helpers for kr0ki's assurance thread (`/assurance/*`):
//   source obligation -> requirement -> system element -> enforcing control -> verification case -> evidence
//
// The server owns every answer and qualifies it with the revisions it was computed at; this module only fetches
// and presents. Two things are deliberately never merged here:
//   * a satisfaction ASSERTION (an architectural claim: some element is said to satisfy the requirement), and
//   * a verification RESULT (revision-specific evidence: a case passed or failed at one model, implementation and
//     configuration revision, and goes stale when any of the three changes).
// A requirement is therefore one of five states, each with its own label, symbol and class, never colour alone.

/** Ordered from "nothing claimed" to "proved at these revisions". */
export const STATES = [
  { id: 'unsatisfied', label: 'not satisfied', symbol: '○', hint: 'Nothing asserts that any system element satisfies it.' },
  { id: 'satisfied_untested', label: 'satisfied · untested', symbol: '◐', hint: 'The design asserts it; no verification case has run.' },
  { id: 'verified', label: 'verified', symbol: '✓', hint: 'A verification case passed at the current revisions and nothing fresh failed.' },
  { id: 'failing', label: 'failing', symbol: '✗', hint: 'A verification case failed (or could not run) at the current revisions.' },
  { id: 'stale', label: 'result stale', symbol: '⟳', hint: 'Evidence exists only for other revisions. Re-run the case.' },
]
const STATE_BY_ID = Object.fromEntries(STATES.map((s) => [s.id, s]))
export const stateInfo = (id) => STATE_BY_ID[id] || { id, label: String(id), symbol: '?', hint: 'Unknown state.' }

export const SOURCE_KINDS = {
  binding_obligation: { label: 'binding', hint: 'A law, contract term or standard clause the system must meet.' },
  organisational_policy: { label: 'policy', hint: "The organisation's own policy." },
  guidance: { label: 'guidance', hint: 'Advice that may be tailored.' },
}
export const sourceKindInfo = (k) => SOURCE_KINDS[k] || { label: String(k || 'unknown'), hint: '' }

const GAPS = {
  no_source: 'No source obligation derives this requirement.',
  unsatisfied: 'No system element is asserted to satisfy it (an implementation gap).',
  no_enforcing_control: 'No control is recorded as enforcing it.',
  control_not_implemented: (g) => `Control ${g.control_id} records no implementation yet.`,
  no_verification_case: 'No verification case verifies it.',
  unknown_verification_case: (g) => `Its verification_id names ${g.verification_id}, which is not a case in the baseline.`,
  satisfied_untested: 'Satisfied by design, but no evidence exists.',
  failing: 'A verification case failed at the current revisions.',
  stale: 'Evidence exists only for other revisions.',
  dangling_element: (g) => `${g.element_id} does not exist at the model revision.`,
}
export const gapKind = (g) => (g && g.gap) || 'unknown'
export function describeGap(g) {
  const d = GAPS[gapKind(g)]
  return typeof d === 'function' ? d(g) : d || `Gap: ${gapKind(g)}`
}

/** `sha256:0213bc9c...` -> `0213bc9c3286`; a git sha -> its first 10 characters; `+dirty.x` is kept visible. */
export function shortRev(rev) {
  if (!rev) return '—'
  const s = String(rev).replace(/^sha256:/, '')
  const [head, dirty] = s.split('+')
  return head.slice(0, head.length > 40 ? 12 : 10) + (dirty ? `+${dirty.slice(0, 11)}` : '')
}

/** Which of the three revisions a stale result no longer matches. */
export function driftText(freshness) {
  if (!freshness || freshness.state !== 'stale') return ''
  const d = freshness.drift || {}
  const changed = ['model', 'implementation', 'configuration'].filter((k) => d[k])
  return changed.length ? `${changed.join(' + ')} changed` : 'revisions changed'
}

export class AssuranceError extends Error {
  constructor(status, body, fallback) {
    super((body && (body.message || body.detail)) || fallback || `HTTP ${status}`)
    this.name = 'AssuranceError'
    this.status = status
    this.code = (body && body.error) || ''
    this.body = body || null
  }
}

/** What to tell the person, by status. Never echoes a token. */
export function explainError(e) {
  if (!(e instanceof AssuranceError)) return `Could not reach the kr0ki server: ${e && e.message ? e.message : e}`
  if (e.status === 401) return 'The server wants an access token. Add one under Setup (kept for this browser session only).'
  if (e.status === 403) return `Your token is not allowed to do this. ${e.message}`
  if (e.status === 503 && e.code === 'assurance_not_configured') return 'The assurance thread is not enabled on this server (set KR0KI_ASSURANCE_BASELINE).'
  if (e.status === 503 && e.code === 'audit_unavailable') return 'The server refused because it cannot write its audit log. Nothing was done.'
  if (e.status === 409 && e.code === 'revision_mismatch') return `The repository moved on: ${e.message}`
  if (e.status === 409 && e.code === 'stale_base') return `Stale base: ${e.message}`
  return e.message
}

const qs = (query) => {
  const p = new URLSearchParams()
  for (const [k, v] of Object.entries(query || {})) if (v !== undefined && v !== null && String(v) !== '') p.set(k, String(v))
  const s = p.toString()
  return s ? `?${s}` : ''
}

/**
 * @param {{ baseUrl: string, token?: string, fetchImpl?: typeof fetch, requestId?: () => string }} deps
 * The token goes only in the Authorization header. It is never put in a URL, a log line or an error message.
 */
export function createAssuranceClient({ baseUrl, token = '', fetchImpl = (...a) => globalThis.fetch(...a), requestId = () => globalThis.crypto?.randomUUID?.() || String(Date.now()) }) {
  const base = String(baseUrl || '').trim().replace(/\/$/, '')
  async function call(method, path, { query, as = 'json' } = {}) {
    const headers = { Accept: as === 'json' ? 'application/json' : '*/*', 'X-Request-ID': requestId() }
    if (token) headers.Authorization = `Bearer ${token}`
    const res = await fetchImpl(`${base}${path}${qs(query)}`, { method, headers })
    if (!res.ok) {
      let body = null
      try { body = await res.json() } catch { /* not JSON */ }
      throw new AssuranceError(res.status, body, res.statusText)
    }
    return as === 'text' ? res.text() : as === 'blob' ? res.blob() : res.json()
  }
  const id = encodeURIComponent
  return {
    list: (filters = {}) => call('GET', '/assurance/requirements', { query: filters }),
    requirement: (rid) => call('GET', `/assurance/requirements/${id(rid)}`),
    trace: (rid) => call('GET', `/assurance/requirements/${id(rid)}/trace`),
    evidence: (filters = {}) => call('GET', '/assurance/evidence', { query: filters }),
    view: () => call('GET', '/assurance/view', { query: { format: 'json' } }),
    viewSvg: () => call('GET', '/assurance/view', { query: { format: 'svg' }, as: 'blob' }),
    viewTable: () => call('GET', '/assurance/view', { query: { format: 'table' }, as: 'text' }),
    verify: (caseId, revision) => call('POST', `/assurance/verify/${id(caseId)}`, { query: { revision } }),
    audit: (filters = {}) => call('GET', '/assurance/audit', { query: filters }),
  }
}
