import { describe, expect, it } from 'vitest'
import { diffLines, diffStats } from '../lineDiff.js'

describe('lineDiff', () => {
  it('marks added, removed and unchanged lines', () => {
    expect(diffLines('a\nb\nc', 'a\nc\nd')).toEqual([
      { op: 'same', text: 'a' }, { op: 'del', text: 'b' }, { op: 'same', text: 'c' }, { op: 'add', text: 'd' },
    ])
  })
  it('counts changes and handles empty sides', () => {
    expect(diffStats('', 'x\ny')).toEqual({ added: 2, removed: 0 })
    expect(diffStats('x\ny', '')).toEqual({ added: 0, removed: 2 })
    expect(diffStats('same', 'same')).toEqual({ added: 0, removed: 0 })
  })
})
