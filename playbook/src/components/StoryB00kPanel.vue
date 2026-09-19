<script setup>
defineProps({
  panel: { type: Object, required: true },
})

const emit = defineEmits(['edit'])

const kindLabel = {
  'query-result': 'Model data',
  render: 'Rendered diagram',
  'sysml-text': 'SysML v2 source',
  narration: 'Agent narration',
}
</script>

<template>
  <article class="storyb00k-panel" :data-kind="panel.kind">
    <span class="storyb00k-panel__label">{{ kindLabel[panel.kind] || 'Agent output' }}</span>
    <div v-if="panel.kind === 'render'" class="storyb00k-panel__render">
      <img v-if="panel.imageDataUrl" data-testid="rendered-image" :src="panel.imageDataUrl" alt="Rendered diagram" />
      <div v-else data-testid="rendered-output" v-html="panel.content"></div>
      <!-- The diagram's source is already in the panel: let the user jump
           straight into the editor with it preloaded. -->
      <div v-if="panel.source?.text" class="storyb00k-panel__source-block">
        <div class="storyb00k-panel__source-head">
          <button
            class="storyb00k-panel__edit"
            data-testid="edit-in-editor"
            :title="`Edit this ${panel.source.format || 'diagram'} source in the editor`"
            @click="emit('edit', { source: panel.source.text, format: panel.source.format })"
          >
            ✏️ Edit
          </button>
          <pre v-if="panel.source?.text" data-testid="source-text">{{ panel.source.text }}</pre>
        </div>
      </div>
    </div>
    <pre v-else-if="panel.kind === 'query-result'">{{ panel.content }}</pre>
    <p v-else>{{ panel.content }}</p>
  </article>
</template>

<style scoped>
.storyb00k-panel { border: 1px solid #c8ced8; border-radius: 6px; padding: .75rem; margin-bottom: .75rem; }
.storyb00k-panel__label { display: block; font-size: .75rem; font-weight: 700; opacity: .7; text-transform: uppercase; margin-bottom: .4rem; }
.storyb00k-panel[data-kind='narration'] { background: #f0f4ff; }
.storyb00k-panel[data-kind='query-result'] { background: #f4fff0; }
.storyb00k-panel__render :deep(svg) { max-width: 100%; height: auto; }
.storyb00k-panel__source-block { margin-top: .5rem; }
.storyb00k-panel__source-head { display: flex; gap: .5rem; align-items: flex-start; }
.storyb00k-panel__source-head pre { flex: 1; margin: 0; white-space: pre-wrap; overflow-x: auto; }
.storyb00k-panel__edit { font-size: .78rem; padding: .15rem .6rem; cursor: pointer; white-space: nowrap; }
</style>
