// Browser history for the playbook's tabs. Without it every tab switch is invisible to the browser, so Back leaves kr0ki
// entirely. Each view gets a history entry (`#/agent`); Back/Forward re-select the previous view.
export const VIEWS = ['gallery', 'planner', 'projects', 'assurance', 'editor', 'storyb00k', 'setup']
const SLUG = { storyb00k: 'agent' } // the tab says "Agent"; the internal id is historical
const FROM_SLUG = Object.fromEntries(Object.entries(SLUG).map(([k, v]) => [v, k]))

export const viewToHash = (view) => `#/${SLUG[view] || view}`

export function hashToView(hash) {
  const slug = String(hash || '').replace(/^#\/?/, '').split(/[/?]/)[0]
  const view = FROM_SLUG[slug] || slug
  return VIEWS.includes(view) ? view : null
}

/**
 * Keep `getView` and the URL in step. `setView` is called for Back/Forward and for a deep link on load. Returns `{push, stop}`:
 * call `push(view)` when the user picks a view (a no-op if it is already the current entry).
 */
export function bindViewHistory({ getView, setView, win = globalThis.window }) {
  const initial = hashToView(win.location.hash)
  if (initial && initial !== getView()) setView(initial)
  // Make the entry we are on carry its view, so Back from the next one lands here.
  win.history.replaceState({ view: getView() }, '', viewToHash(getView()))
  const onPop = () => {
    const v = hashToView(win.location.hash)
    if (v && v !== getView()) setView(v)
  }
  win.addEventListener('popstate', onPop)
  return {
    push(view) {
      if (hashToView(win.location.hash) === view) return
      win.history.pushState({ view }, '', viewToHash(view))
    },
    stop: () => win.removeEventListener('popstate', onPop),
  }
}
