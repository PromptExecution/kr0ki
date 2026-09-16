<script setup>
import Gallery from './Gallery.vue'

const examples = [
  {
    id: 'graphviz-pipeline',
    format: 'graphviz',
    title: 'IAC to artifact pipeline',
    input_kind: 'IAC / code',
    description: 'A visual-regression-ready example panel.',
    source: 'digraph { iac -> diagram -> kroki -> artifact }',
    outputs: ['svg', 'png'],
  },
  {
    id: 'nomnoml-components',
    format: 'nomnoml',
    title: 'Renderer components',
    input_kind: 'component model',
    description: 'A concise component relation view of the render loop.',
    source: '[IAC / code]->[RenderService]',
    outputs: ['svg', 'png'],
  },
]
</script>

<template>
  <Story title="Playb00k / Gallery">
    <Variant title="Two-card catalog">
      <Gallery :examples="examples" />
    </Variant>
  </Story>
</template>
