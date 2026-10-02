<script setup>
// The left-hand catalog: diagram types grouped by what they are for, as a Quasar tree. Selecting a type opens
// it in the Code Editor, which renders it automatically. Fixtures that no type card references live under
// "Other fixtures" so every example stays reachable from here.
import { computed, ref, watch } from 'vue'
import { QTree } from 'quasar' // imported by name: only the tree is bundled, not the whole framework

const props = defineProps({
  catalog: { type: Object, default: null },
  examples: { type: Array, default: () => [] },
  selectedExampleId: { type: String, default: '' },
  suggested: { type: Array, default: () => [] },
})
const emit = defineEmits(['select'])

// QTree's default expand arrow is the Material Icons ligature `play_arrow`; with no icon font bundled (air-gapped) it rendered
// as the literal text "play_arrow". An inline SVG needs no font and no network.
const ARROW_ICON = `img:data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#9fb0d0" d="M8 5v14l11-7z"/></svg>')}`

const OTHER = 'group:other'
const groupKey = (name) => `group:${name}`

const nodes = computed(() => {
  const types = props.catalog?.types || []
  const exampleIds = new Set(types.map((t) => t.exampleId))
  const groups = (props.catalog?.useCases || []).map((uc) => {
    const members = types.filter((t) => t.useCases.includes(uc))
    return {
      key: groupKey(uc),
      label: `${uc} (${members.length})`,
      selectable: false,
      children: members.map((t) => ({
        key: `type:${uc}/${t.id}`,
        label: t.name,
        syntax: t.syntax,
        typeId: t.id,
        exampleId: t.exampleId,
        header: 'leaf',
      })),
    }
  })
  const others = props.examples.filter((e) => !exampleIds.has(e.id))
  if (others.length) {
    groups.push({
      key: OTHER,
      label: `other fixtures (${others.length})`,
      selectable: false,
      children: others.map((e) => ({ key: `fixture:${e.id}`, label: e.title, syntax: e.format, exampleId: e.id, header: 'leaf' })),
    })
  }
  return groups
})

const expanded = ref([])
watch(
  () => props.catalog,
  (c) => {
    if (c) expanded.value = nodes.value.filter((n) => n.key !== OTHER).map((n) => n.key)
  },
  { immediate: true },
)

const leaves = computed(() => nodes.value.flatMap((g) => g.children))
const selectedKey = computed(() => leaves.value.find((l) => l.exampleId === props.selectedExampleId)?.key ?? null)
const suggestedSet = computed(() => new Set(props.suggested))

function onSelected(key) {
  if (!key) return
  const leaf = leaves.value.find((l) => l.key === key)
  if (leaf) emit('select', { exampleId: leaf.exampleId, typeId: leaf.typeId || '' })
}
</script>

<template>
  <nav class="catalog-tree" aria-label="Diagram types" data-testid="catalog-tree">
    <p class="catalog-heading">Diagram types</p>
    <QTree
      v-if="nodes.length"
      v-model:expanded="expanded"
      :selected="selectedKey"
      :nodes="nodes"
      :icon="ARROW_ICON"
      node-key="key"
      dense
      dark
      no-connectors
      no-selection-unset
      @update:selected="onSelected"
    >
      <template #default-header="{ node }">
        <span v-if="node.header === 'leaf'" class="tree-leaf" :class="{ suggested: suggestedSet.has(node.typeId) }" :data-type-id="node.typeId || null" :title="`${node.label} (${node.syntax})`">
          <span class="tree-name"><span v-if="suggestedSet.has(node.typeId)" class="tree-star">★</span>{{ node.label }}</span><span class="tree-syntax">{{ node.syntax }}</span>
        </span>
        <span v-else class="tree-group">{{ node.label }}</span>
      </template>
    </QTree>
    <p v-else class="catalog-empty">Loading types…</p>
  </nav>
</template>

<style scoped>
.catalog-tree { display: block; min-height: 0; overflow-y: auto; flex: 1; font-size: .74rem; line-height: 1.15; }
.catalog-heading { margin: 0 0 .15rem; color: #7f8db2; font-size: .68rem; font-weight: 800; text-transform: uppercase; letter-spacing: .06em; }
.catalog-empty { color: #6f7fa3; font-size: .74rem; }
.tree-group { text-transform: capitalize; color: #9fb0d0; font-weight: 700; font-size: .72rem; }
.tree-leaf { display: flex; align-items: baseline; gap: .4rem; width: 100%; min-width: 0; color: #ced8ee; }
.tree-name { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree-leaf.suggested { color: #e9d5ff; }
.tree-star { color: #c084fc; margin-right: .2rem; }
.tree-syntax { flex: none; color: #6f7fa3; font-size: .64rem; }
/* tight rows: Quasar's dense tree still pads generously */
.catalog-tree :deep(.q-tree__node-header) { padding: 1px 2px; min-height: 0; }
.catalog-tree :deep(.q-tree__node) { padding: 0 0 0 .55rem; }
.catalog-tree > :deep(.q-tree) > .q-tree__node { padding-left: 0; }
.catalog-tree :deep(.q-tree__arrow) { font-size: .9rem; margin-right: 2px; }
.catalog-tree :deep(.q-tree__node-header-content) { color: inherit; min-width: 0; overflow: hidden; }
.catalog-tree :deep(.q-tree__node--selected > .q-tree__node-header) { background: #243465; border-radius: .3rem; }
.catalog-tree :deep(.q-tree__node-header:hover) { background: #1b2a52; border-radius: .3rem; }
.catalog-tree :deep(.q-tree__node-header.q-tree__node--link) { cursor: pointer; }
</style>
