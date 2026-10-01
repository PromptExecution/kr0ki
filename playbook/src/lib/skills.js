// The syntax skill the diagram agent loads before rendering a language, fetched from the agent
// (GET /skills/diagrams/<format>) so the editor can show the same guidance next to the source.
const cache = new Map()

/** @returns {Promise<{status: 'ok', text: string} | {status: 'missing'} | {status: 'error', message: string}>} */
export async function fetchSkill(agentUrl, format, fetchImpl = globalThis.fetch) {
  if (!agentUrl || !format) return { status: 'error', message: 'no agent configured' }
  const key = `${agentUrl}|${format}`
  if (cache.has(key)) return cache.get(key)
  try {
    const res = await fetchImpl(`${agentUrl.replace(/\/$/, '')}/skills/diagrams/${encodeURIComponent(format)}`)
    if (res.status === 404) return remember(key, { status: 'missing' })
    if (!res.ok) return { status: 'error', message: `agent returned ${res.status}` }
    return remember(key, { status: 'ok', text: await res.text() })
  } catch (err) {
    // not cached: the agent may simply not be running yet
    return { status: 'error', message: err?.message || 'agent unreachable' }
  }
}

function remember(key, value) {
  cache.set(key, value)
  return value
}

export function clearSkillCache() {
  cache.clear()
}
