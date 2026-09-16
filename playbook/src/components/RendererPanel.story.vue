<script setup>
import RendererPanel from './RendererPanel.vue'

const graphviz = {
  id: 'graphviz-pipeline',
  format: 'graphviz',
  title: 'IAC to artifact pipeline',
  input_kind: 'IAC / code',
  description: 'A visual-regression-ready example panel.',
  source: 'digraph { iac -> diagram -> kroki -> artifact }',
  outputs: ['svg', 'png'],
}
</script>

<template>
  <Story title="Playb00k / Renderer panel">
    <Variant title="GraphViz fixture">
      <RendererPanel :example="graphviz" :examples="[graphviz]" />
    </Variant>
  </Story>
</template>
