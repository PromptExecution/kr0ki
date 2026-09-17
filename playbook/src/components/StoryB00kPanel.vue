<script setup>
defineProps({
  panel: { type: Object, required: true },
})

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
      <div data-testid="rendered-output" v-html="panel.content"></div>
      <pre v-if="panel.source?.source || panel.source?.text" data-testid="source-text">{{ panel.source.source || panel.source.text }}</pre>
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
</style>
