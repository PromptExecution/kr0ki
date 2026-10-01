import { beforeEach, describe, expect, it, vi } from 'vitest'
import { clearSkillCache, fetchSkill } from '../skills.js'

const reply = (status, text = '') => ({ status, ok: status >= 200 && status < 300, text: async () => text })

describe('fetchSkill', () => {
  beforeEach(() => clearSkillCache())

  it('fetches the skill text from the agent and caches it', async () => {
    const f = vi.fn(async () => reply(200, '---\nname: kr0ki-d2\n---\n# d2'))
    expect(await fetchSkill('http://a:8789/', 'd2', f)).toEqual({ status: 'ok', text: '---\nname: kr0ki-d2\n---\n# d2' })
    expect(f).toHaveBeenCalledWith('http://a:8789/skills/diagrams/d2')
    await fetchSkill('http://a:8789/', 'd2', f)
    expect(f).toHaveBeenCalledTimes(1)
  })

  it('reports a missing skill (and remembers it) separately from a failure (which it does not)', async () => {
    const f = vi.fn(async () => reply(404))
    expect(await fetchSkill('http://a', 'k8s-topology', f)).toEqual({ status: 'missing' })
    await fetchSkill('http://a', 'k8s-topology', f)
    expect(f).toHaveBeenCalledTimes(1)
    const down = vi.fn(async () => { throw new Error('connection refused') })
    expect(await fetchSkill('http://a', 'd2', down)).toEqual({ status: 'error', message: 'connection refused' })
    await fetchSkill('http://a', 'd2', down)
    expect(down).toHaveBeenCalledTimes(2) // an unreachable agent may come up later
    expect(await fetchSkill('http://a', 'd2', vi.fn(async () => reply(500)))).toEqual({ status: 'error', message: 'agent returned 500' })
  })

  it('needs an agent and a format, and escapes the format into the path', async () => {
    expect((await fetchSkill('', 'd2')).status).toBe('error')
    expect((await fetchSkill('http://a', '')).status).toBe('error')
    const f = vi.fn(async () => reply(404))
    await fetchSkill('http://a', '../x', f)
    expect(f).toHaveBeenCalledWith('http://a/skills/diagrams/..%2Fx')
  })
})
