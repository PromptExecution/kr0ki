<script setup>
import StoryB00kPanel from './StoryB00kPanel.vue'

const panels = [
  { kind: 'narration', content: 'The live model has one deployment engine, two backing services, and a shared config surface. Ask me to render any of them.' },
  { kind: 'query-result', content: '[\n  { "element": "DeploymentEngine", "kind": "part", "namespace": "demo" },\n  { "element": "ConfigSurface", "kind": "reference", "namespace": "demo" }\n]' },
  { kind: 'render', content: '<svg viewBox="0 0 120 40" xmlns="http://www.w3.org/2000/svg"><rect width="118" height="38" x="1" y="1" rx="4" fill="#eef3ff" stroke="#5271ff"/><text x="60" y="24" text-anchor="middle" font-size="12" fill="#233">demo::DeploymentEngine</text></svg>', source: { text: 'deployment: demo::DeploymentEngine { shape: rectangle }', format: 'd2' } },
  { kind: 'render', content: '', imageDataUrl: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==', source: { text: 'graph -> png', format: 'graphviz' } },
]
</script>

<template>
  <Story title="Playb00k / StoryB00k panel">
    <Variant title="All four panel kinds">
      <div style="max-width: 34rem; padding: 1rem;">
        <StoryB00kPanel v-for="(panel, i) in panels" :key="i" :panel="panel" />
      </div>
    </Variant>
    <Variant title="Narration only">
      <div style="max-width: 34rem; padding: 1rem;">
        <StoryB00kPanel :panel="panels[0]" />
      </div>
    </Variant>
    <Variant title="Binary render (PNG image element)">
      <div style="max-width: 34rem; padding: 1rem;">
        <StoryB00kPanel :panel="panels[3]" />
      </div>
    </Variant>
  </Story>
</template>
