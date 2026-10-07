// Line-level diff (LCS) for comparing diagram sources between revisions. Sources are small (hundreds of lines), so the
// O(n*m) table is fine; inputs over MAX_LINES fall back to "everything changed" instead of allocating a huge table.
const MAX_LINES = 2000

/** @returns {{op: 'same'|'add'|'del', text: string}[]} */
export function diffLines(a = '', b = '') {
  const x = a === '' ? [] : String(a).split('\n')
  const y = b === '' ? [] : String(b).split('\n')
  if (x.length > MAX_LINES || y.length > MAX_LINES) {
    return [...x.map((text) => ({ op: 'del', text })), ...y.map((text) => ({ op: 'add', text }))]
  }
  const n = x.length
  const m = y.length
  const t = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1))
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) t[i][j] = x[i] === y[j] ? t[i + 1][j + 1] + 1 : Math.max(t[i + 1][j], t[i][j + 1])
  const out = []
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (x[i] === y[j]) { out.push({ op: 'same', text: x[i] }); i++; j++ }
    else if (t[i + 1][j] >= t[i][j + 1]) out.push({ op: 'del', text: x[i++] })
    else out.push({ op: 'add', text: y[j++] })
  }
  while (i < n) out.push({ op: 'del', text: x[i++] })
  while (j < m) out.push({ op: 'add', text: y[j++] })
  return out
}

export function diffStats(a, b) {
  const d = diffLines(a, b)
  return { added: d.filter((l) => l.op === 'add').length, removed: d.filter((l) => l.op === 'del').length }
}
