<script setup>
// The diagram-source editor: CodeMirror 6, top-aligned, with syntax highlighting where a language exists and an optional
// LSP connection (completion, hover, diagnostics) when a language server is configured for the format.
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Compartment, EditorState } from '@codemirror/state'
import { EditorView, drawSelection, highlightActiveLine, keymap, lineNumbers } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { HighlightStyle, bracketMatching, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'
import { connectLsp, highlightFor } from '../lib/lsp.js'

const props = defineProps({
  modelValue: { type: String, default: '' },
  format: { type: String, default: '' },
  lspUrl: { type: String, default: '' },
})
const emit = defineEmits(['update:modelValue', 'lsp-status'])

const host = ref(null)
let view = null
const languageSlot = new Compartment()
const lspSlot = new Compartment()
let lspSession = null
let lspRun = 0

const theme = EditorView.theme(
  {
    '&': { height: '100%', backgroundColor: '#091127', color: '#edf5ff', border: '1px solid #3b4d7d', borderRadius: '.45rem' },
    '.cm-scroller': { fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace', fontSize: '.8rem', lineHeight: '1.5', overflow: 'auto' },
    '.cm-content': { caretColor: '#38bdf8' },
    '.cm-gutters': { backgroundColor: '#0b1226', color: '#6f7fa3', border: 'none', borderRight: '1px solid #2a3966' },
    '.cm-activeLine': { backgroundColor: 'rgba(56,189,248,.07)' },
    '.cm-activeLineGutter': { backgroundColor: 'rgba(56,189,248,.12)' },
    '&.cm-focused': { outline: '1px solid #38bdf8' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'rgba(56,189,248,.28)' },
    '.cm-tooltip': { backgroundColor: '#131d3c', border: '1px solid #3b4d7d', color: '#edf5ff' },
    '.cm-diagnostic-error': { borderLeft: '3px solid #f87171' },
    '.cm-diagnostic-warning': { borderLeft: '3px solid #fbbf24' },
  },
  { dark: true },
)
const highlight = HighlightStyle.define([
  { tag: [t.propertyName, t.attributeName], color: '#7dd3fc' },
  { tag: [t.string, t.special(t.string)], color: '#86efac' },
  { tag: [t.number, t.bool, t.null], color: '#fbbf24' },
  { tag: [t.keyword, t.operatorKeyword], color: '#c084fc' },
  { tag: t.comment, color: '#6f7fa3', fontStyle: 'italic' },
  { tag: [t.punctuation, t.separator, t.brace, t.squareBracket], color: '#94a6c8' },
])

function languageExtensions(format) {
  return highlightFor(format) || []
}

async function attachLsp() {
  const run = ++lspRun
  lspSession?.dispose?.()
  lspSession = null
  view?.dispatch({ effects: lspSlot.reconfigure([]) })
  if (!props.lspUrl) return emit('lsp-status', { status: 'none' })
  emit('lsp-status', { status: 'connecting' })
  const result = await connectLsp(props.format, props.lspUrl, { documentUri: `file:///kr0ki/source.${props.format}` })
  if (run !== lspRun) return result.dispose?.() // the format or URL changed while we were connecting
  if (result.status === 'connected' && view) {
    lspSession = result
    view.dispatch({ effects: lspSlot.reconfigure(result.extension) })
  }
  emit('lsp-status', { status: result.status, message: result.message, languageId: result.languageId })
}

onMounted(() => {
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        lineNumbers(), history(), drawSelection(), highlightActiveLine(), bracketMatching(),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        theme, syntaxHighlighting(highlight),
        languageSlot.of(languageExtensions(props.format)),
        lspSlot.of([]),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) emit('update:modelValue', u.state.doc.toString())
        }),
      ],
    }),
  })
  attachLsp()
})

// v-model in: replace the document only when it really differs (our own edits come back as the same text)
watch(() => props.modelValue, (next) => {
  if (view && next !== view.state.doc.toString()) {
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next ?? '' } })
  }
})
watch(() => props.format, (f) => {
  view?.dispatch({ effects: languageSlot.reconfigure(languageExtensions(f)) })
  attachLsp()
})
watch(() => props.lspUrl, attachLsp)

onBeforeUnmount(() => {
  lspRun++
  lspSession?.dispose?.()
  view?.destroy()
  view = null
})

defineExpose({ focus: () => view?.focus(), getText: () => view?.state.doc.toString() ?? '' })
</script>

<template>
  <div ref="host" class="code-editor" data-testid="source-editor" :data-format="format" />
</template>

<style scoped>
.code-editor { height: 20rem; min-height: 10rem; resize: vertical; overflow: hidden; margin-top: .4rem; }
</style>
