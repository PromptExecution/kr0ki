import { describe, expect, it, vi } from 'vitest'
import { renderSvg, sourceFilename } from '../renderSource.js'

describe('renderSvg', () => {
  it('posts the source to the format route and returns the svg', async () => {
    const f = vi.fn(async () => ({ ok: true, text: async () => '<svg/>' }))
    expect(await renderSvg({ rendererUrl: 'http://k/', format: 'd2', source: 'a->b', fetchImpl: f })).toBe('<svg/>')
    expect(f).toHaveBeenCalledWith('http://k/render/d2?output=svg', expect.objectContaining({ method: 'POST', body: 'a->b' }))
  })
  it('uses a custom route and reports server errors', async () => {
    const f = vi.fn(async () => ({ ok: false, status: 400, text: async () => 'bad source' }))
    await expect(renderSvg({ rendererUrl: 'http://k', format: 'k8s-topology', route: '/render/k8s-topology', source: 'x', fetchImpl: f })).rejects.toThrow('bad source')
    expect(f.mock.calls[0][0]).toBe('http://k/render/k8s-topology?output=svg')
  })
  it('refuses without a URL or format', async () => {
    await expect(renderSvg({ rendererUrl: '', format: 'd2', source: 'x' })).rejects.toThrow('Setup')
    await expect(renderSvg({ rendererUrl: 'http://k', format: '', source: 'x' })).rejects.toThrow('format')
  })
  it('picks a file extension', () => {
    expect(sourceFilename('d2')).toBe('diagram.d2')
    expect(sourceFilename('graphviz', 'g')).toBe('g.dot')
    expect(sourceFilename('weird')).toBe('diagram.weird')
  })
})
