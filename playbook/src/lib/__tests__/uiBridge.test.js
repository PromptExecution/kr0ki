import { describe, expect, it, vi } from 'vitest'
import { connectUiBridge, parseCommand, plannerSessionId } from '../uiBridge.js'

const env = (seq, command) => JSON.stringify({ seq, command })

class FakeEventSource {
  static last
  constructor(url) { this.url = url; this.listeners = {}; this.readyState = 0; FakeEventSource.last = this }
  addEventListener(type, fn) { this.listeners[type] = fn }
  emit(data) { this.listeners.ui?.({ data }) }
  close() { this.readyState = 2; this.closed = true }
}

describe('parseCommand', () => {
  it('accepts exactly the documented commands', () => {
    expect(parseCommand(env(1, { type: 'open_view', view: 'agent' }))).toMatchObject({ type: 'open_view', view: 'agent' })
    expect(parseCommand(env(2, { type: 'filter_gallery', use_case: null }))).toMatchObject({ useCase: 'All' })
    expect(parseCommand(env(3, { type: 'filter_gallery', use_case: 'chart' }))).toMatchObject({ useCase: 'chart' })
    expect(parseCommand(env(4, { type: 'select_type', type_id: 'class' }))).toMatchObject({ typeId: 'class' })
    expect(parseCommand(env(5, { type: 'suggest', type_ids: ['a', 'b'], note: 'why' }))).toMatchObject({ typeIds: ['a', 'b'], note: 'why' })
  })

  it('ignores malformed or unknown commands rather than guessing', () => {
    for (const bad of [
      'not json', '{}', env(1, null), env(1, { type: 'eval', code: 'alert(1)' }),
      env(1, { type: 'open_view', view: 'admin' }), env(1, { type: 'select_type', type_id: '' }),
      env(1, { type: 'select_type', type_id: 7 }), env(1, { type: 'suggest', type_ids: [1] }),
      env(1, { type: 'filter_gallery', use_case: 5 }),
    ]) expect(parseCommand(bad)).toBeNull()
  })
})

describe('connectUiBridge', () => {
  it('subscribes to the session url and delivers each command once, in order', () => {
    const got = []; const status = []
    connectUiBridge({ baseUrl: 'http://h:8787/', sessionId: 'planner-abc', onCommand: (c) => got.push(c), onStatus: (s) => status.push(s), EventSourceImpl: FakeEventSource })
    const es = FakeEventSource.last
    expect(es.url).toBe('http://h:8787/ui/planner-abc/events')
    es.emit(env(1, { type: 'open_view', view: 'gallery' }))
    es.emit(env(2, { type: 'select_type', type_id: 'class' }))
    es.emit(env(2, { type: 'select_type', type_id: 'class' })) // replay after reconnect
    es.emit(env(1, { type: 'open_view', view: 'gallery' }))
    es.emit('garbage')
    expect(got.map((c) => c.type)).toEqual(['open_view', 'select_type'])
    expect(status).toEqual(['connecting'])
  })

  it('reports status, closes cleanly, and degrades to a no-op without EventSource or a session', () => {
    const status = vi.fn()
    const b = connectUiBridge({ baseUrl: 'http://h', sessionId: 's', onCommand() {}, onStatus: status, EventSourceImpl: FakeEventSource })
    FakeEventSource.last.onopen()
    b.close()
    expect(FakeEventSource.last.closed).toBe(true)
    expect(status.mock.calls.map((c) => c[0])).toEqual(['connecting', 'open', 'closed'])
    const none = vi.fn()
    connectUiBridge({ baseUrl: 'http://h', sessionId: '', onCommand() {}, onStatus: none, EventSourceImpl: FakeEventSource }).close()
    expect(none).toHaveBeenCalledWith('closed')
  })
})

describe('plannerSessionId', () => {
  const mem = () => { const m = {}; return { getItem: (k) => m[k] ?? null, setItem: (k, v) => { m[k] = v } } }

  it('is stable per tab, server-valid, and prefixed so the agent treats the thread as a planner', () => {
    const storage = mem()
    const a = plannerSessionId(storage, () => 'ABCD-1234-uuid')
    expect(a).toMatch(/^planner-[A-Za-z0-9_-]{1,56}$/)
    expect(a.length).toBeLessThanOrEqual(64)
    expect(plannerSessionId(storage, () => 'other')).toBe(a)
  })

  it('survives blocked storage and sanitizes odd random sources', () => {
    const blocked = { getItem() { throw new Error('denied') }, setItem() { throw new Error('denied') } }
    expect(plannerSessionId(blocked, () => 'x'.repeat(100) + '!!')).toMatch(/^planner-x{1,56}$/)
    expect(plannerSessionId(blocked, () => undefined)).toMatch(/^planner-[A-Za-z0-9_-]+$/)
  })

  it('discards a stored id that the server would reject', () => {
    const storage = mem(); storage.setItem('kr0ki:plannerSession', 'planner-bad id!')
    expect(plannerSessionId(storage, () => 'fresh')).toBe('planner-fresh')
  })
})
