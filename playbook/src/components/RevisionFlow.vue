<script setup>
// Visual revision DAG (vue-flow): every prompt/edit/fork is a node; click to
// time-travel, fork button branches, active node is highlighted. Layout is a
// simple vertical tree (children under parents) — deterministic, no dagre dep.
import { computed, watch } from 'vue'
import { VueFlow, Handle, Position, useVueFlow } from '@vue-flow/core'
import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'

const props = defineProps({
  graph: { type: Object, required: true },
})
const emit = defineEmits(['checkout', 'fork'])

const { fitView } = useVueFlow()

const kindColor = { root: '#2563eb', prompt: '#059669', edit: '#7c3aed', fork: '#d97706' }

// Depth/branch layout: BFS from roots, one column per node at each depth slot.
const layout = computed(() => {
  const nodes = props.graph.nodes
  const byId = new Map(nodes.map((n) => [n.id, n]))
  const children = new Map()
  const roots = []
  for (const n of nodes) {
    if (!n.parentId || !byId.has(n.parentId)) roots.push(n)
    else children.set(n.parentId, [...(children.get(n.parentId) || []), n])
  }
  const positioned = new Map()
  const depthCount = new Map()
  function place(node, depth) {
    const slot = depthCount.get(depth) || 0
    depthCount.set(depth, slot + 1)
    positioned.set(node.id, { x: slot * 240, y: depth * 130 })
    for (const child of children.get(node.id) || []) place(child, depth + 1)
  }
  roots.forEach((r) => place(r, 0))
  return positioned
})

const flowNodes = computed(() =>
  props.graph.nodes.map((n) => {
    const pos = layout.value.get(n.id) || { x: 0, y: 0 }
    const isActive = n.id === props.graph.activeId
    return {
      id: n.id,
      type: 'revision',
      position: pos,
      data: { node: n, isActive, kindColor: kindColor[n.kind] || '#64748b' },
    }
  }),
)

const flowEdges = computed(() =>
  props.graph.nodes
    .filter((n) => n.parentId)
    .map((n) => ({
      id: `e-${n.parentId}-${n.id}`,
      source: n.parentId,
      target: n.id,
      animated: n.id === props.graph.activeId,
      style: { stroke: n.id === props.graph.activeId ? '#2563eb' : '#94a3b8' },
    })),
)

watch(() => props.graph.activeId, () => fitView({ padding: 0.2, duration: 300 }))

function onNodeClick({ node }) {
  emit('checkout', node.id)
}

function onForkClick(nodeId) {
  emit('fork', nodeId)
}
</script>

<template>
  <div class="revflow" data-testid="revision-flow">
    <VueFlow
      :nodes="flowNodes"
      :edges="flowEdges"
      :fit-view-on-init="true"
      :min-zoom="0.2"
      @node-click="onNodeClick"
    >
      <template #node-revision="{ data }">
        <div
          class="revflow__node"
          :class="{ 'revflow__node--active': data.isActive }"
          :style="{ borderColor: data.kindColor }"
          :data-node-kind="data.node.kind"
        >
          <span class="revflow__node-kind" :style="{ background: data.kindColor }">{{ data.node.kind }}</span>
          <p class="revflow__node-label" :title="data.node.prompt || data.node.label">{{ data.node.label }}</p>
          <div class="revflow__node-actions">
            <button v-if="!data.isActive" class="revflow__btn" title="Time travel to this state" @click.stop="$emit('checkout', data.node.id)">⏱</button>
            <button class="revflow__btn" title="Fork a new branch from here" @click.stop="onForkClick(data.node.id)">⑂</button>
          </div>
        </div>
        <Handle type="target" :position="Position.Top" />
        <Handle type="source" :position="Position.Bottom" />
      </template>
    </VueFlow>
  </div>
</template>

<style scoped>
.revflow { height: 320px; border: 1px solid #cbd5e1; border-radius: 6px; background: #f8fafc; }
.revflow__node { background: white; border: 2px solid; border-radius: 8px; padding: .4rem .5rem; min-width: 180px; max-width: 200px; box-shadow: 0 1px 3px rgb(0 0 0 / .1); }
.revflow__node--active { outline: 3px solid #2563eb; box-shadow: 0 0 0 4px rgb(37 99 235 / .15); }
.revflow__node-kind { display: inline-block; color: white; font-size: .65rem; padding: 0 .4rem; border-radius: 999px; text-transform: uppercase; }
.revflow__node-label { margin: .25rem 0; font-size: .78rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.revflow__node-actions { display: flex; gap: .25rem; }
.revflow__btn { font-size: .75rem; border: 1px solid #cbd5e1; background: white; border-radius: 4px; cursor: pointer; padding: 0 .35rem; }
.revflow__btn:hover { background: #eff6ff; }
</style>
