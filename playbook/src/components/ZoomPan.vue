<script setup>
// A viewport for a diagram that may not fit the page: drag to pan, Ctrl/⌘+wheel (or pinch) to zoom at the cursor, buttons and keys for
// zoom in/out, fit and 100%. The content is a slot (inline <svg> or an <img>); `contentKey` changes when it is replaced, which refits it.
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { fit, panBy, svgSize, zoomAt } from '../lib/zoomPan.js'

const props = defineProps({
  contentKey: { type: [String, Number], default: '' },
  /** viewport height; "tall" gives a large working area for big diagrams */
  height: { type: String, default: '24rem' },
  /** natural size {w,h} when the slot cannot report it (an <img> of an svg that has a viewBox but no width/height) */
  size: { type: Object, default: null },
})

const vp = ref(null)
const inner = ref(null)
const view = ref({ scale: 1, x: 0, y: 0 })
const tall = ref(false)
const natural = ref({ w: 0, h: 0 })
let drag = null
const pointers = new Map()
let pinch = 0

function measure() {
  const el = inner.value
  if (!el) return
  const svg = el.querySelector('svg')
  const declared = svg ? svgSize(svg.outerHTML.slice(0, 600)) : null
  const img = el.querySelector('img')
  natural.value = props.size?.w > 0 && props.size?.h > 0
    ? { w: props.size.w, h: props.size.h }
    : declared
    ? { w: declared.w, h: declared.h }
    : img && img.naturalWidth
      ? { w: img.naturalWidth, h: img.naturalHeight }
      : { w: el.scrollWidth || 0, h: el.scrollHeight || 0 }
  // Give an <svg> its natural pixel size so the transform scales it, instead of the svg shrinking to the viewport width.
  // An <img> of a viewBox-only svg has no intrinsic size; pin it to the natural size so the transform scales a real box.
  if (img && natural.value.w > 0) { img.style.width = `${natural.value.w}px`; img.style.height = `${natural.value.h}px` }
  if (svg && declared) { svg.setAttribute('width', declared.w); svg.setAttribute('height', declared.h); svg.style.maxWidth = 'none' }
}

function fitView() {
  const r = vp.value?.getBoundingClientRect()
  if (!r) return
  view.value = fit(natural.value.w, natural.value.h, r.width, r.height)
}
function actual() {
  const r = vp.value?.getBoundingClientRect()
  if (!r) return
  view.value = { scale: 1, x: Math.max(8, (r.width - natural.value.w) / 2), y: 8 }
}
function zoomBy(factor, px, py) {
  const r = vp.value?.getBoundingClientRect()
  view.value = zoomAt(view.value, factor, px ?? (r ? r.width / 2 : 0), py ?? (r ? r.height / 2 : 0))
}
async function refit() {
  await nextTick()
  measure()
  fitView()
}

function onWheel(e) {
  if (!(e.ctrlKey || e.metaKey)) return // plain wheel keeps scrolling the page
  e.preventDefault()
  const r = vp.value.getBoundingClientRect()
  zoomBy(Math.exp(-e.deltaY * 0.0025), e.clientX - r.left, e.clientY - r.top)
}
function onDown(e) {
  vp.value.setPointerCapture?.(e.pointerId)
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })
  if (pointers.size === 1) drag = { x: e.clientX, y: e.clientY }
  if (pointers.size === 2) { drag = null; pinch = dist() }
}
function dist() {
  const [a, b] = [...pointers.values()]
  return Math.hypot(a.x - b.x, a.y - b.y)
}
function onMove(e) {
  if (!pointers.has(e.pointerId)) return
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })
  if (pointers.size === 2 && pinch) {
    const d = dist()
    const [a, b] = [...pointers.values()]
    const r = vp.value.getBoundingClientRect()
    zoomBy(d / pinch, (a.x + b.x) / 2 - r.left, (a.y + b.y) / 2 - r.top)
    pinch = d
  } else if (drag) {
    view.value = panBy(view.value, e.clientX - drag.x, e.clientY - drag.y)
    drag = { x: e.clientX, y: e.clientY }
  }
}
function onUp(e) {
  pointers.delete(e.pointerId)
  drag = pointers.size === 1 ? { ...[...pointers.values()][0] } : null
  pinch = 0
}
function onKey(e) {
  const k = e.key
  if (k === '+' || k === '=') zoomBy(1.25)
  else if (k === '-' || k === '_') zoomBy(0.8)
  else if (k === '0') actual()
  else if (k === 'f' || k === 'F') fitView()
  else if (k === 'ArrowLeft') view.value = panBy(view.value, 40, 0)
  else if (k === 'ArrowRight') view.value = panBy(view.value, -40, 0)
  else if (k === 'ArrowUp') view.value = panBy(view.value, 0, 40)
  else if (k === 'ArrowDown') view.value = panBy(view.value, 0, -40)
  else return
  e.preventDefault()
}

let ro = null
onMounted(() => {
  refit()
  // Content arrives asynchronously (v-html, <img> load): refit whenever the slot's size changes the first times.
  if (typeof ResizeObserver !== 'undefined' && inner.value) {
    let n = 0
    ro = new ResizeObserver(() => { if (n++ < 3) refit() })
    ro.observe(inner.value)
  }
})
onBeforeUnmount(() => ro?.disconnect())
watch(() => props.contentKey, refit)
watch(() => props.size, refit)
watch(tall, () => nextTick(fitView))

const percent = () => `${Math.round(view.value.scale * 100)}%`
defineExpose({ fitView, actual, zoomBy, view })
</script>

<template>
  <div class="zp" data-testid="zoompan">
    <div class="zp__bar">
      <button type="button" data-testid="zp-out" title="Zoom out (−)" @click="zoomBy(0.8)">−</button>
      <span class="zp__pct" data-testid="zp-pct" aria-live="polite">{{ percent() }}</span>
      <button type="button" data-testid="zp-in" title="Zoom in (+)" @click="zoomBy(1.25)">+</button>
      <button type="button" data-testid="zp-fit" title="Fit the whole diagram in view (F)" @click="fitView">Fit</button>
      <button type="button" data-testid="zp-100" title="Actual size (0)" @click="actual">100%</button>
      <button type="button" data-testid="zp-tall" :aria-pressed="tall" title="Use a larger viewing area" @click="tall = !tall">{{ tall ? 'Smaller' : 'Larger' }}</button>
      <span class="zp__hint">drag to pan · Ctrl+wheel or pinch to zoom</span>
    </div>
    <div
      ref="vp"
      class="zp__viewport"
      :style="{ height: tall ? '80vh' : height }"
      tabindex="0"
      role="img"
      aria-label="Diagram viewer. Plus and minus zoom, F fits, 0 is actual size, arrow keys pan."
      @wheel="onWheel"
      @pointerdown="onDown"
      @pointermove="onMove"
      @pointerup="onUp"
      @pointercancel="onUp"
      @keydown="onKey"
      @dblclick="fitView"
    >
      <div ref="inner" class="zp__content" :style="{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})` }">
        <slot />
      </div>
    </div>
  </div>
</template>

<style scoped>
.zp { display: flex; flex-direction: column; gap: .3rem; min-width: 0; }
.zp__bar { display: flex; flex-wrap: wrap; align-items: center; gap: .3rem; font-size: .8rem; }
.zp__bar button { padding: .05rem .5rem; cursor: pointer; }
.zp__pct { min-width: 3rem; text-align: center; font-variant-numeric: tabular-nums; }
.zp__hint { opacity: .6; font-size: .72rem; margin-left: auto; }
.zp__viewport { position: relative; overflow: hidden; border: 1px solid #c8ced8; border-radius: 6px; background: repeating-conic-gradient(rgba(100, 116, 139, .08) 0% 25%, transparent 0% 50%) 0 0 / 16px 16px; cursor: grab; touch-action: none; outline-offset: 2px; }
.zp__viewport:active { cursor: grabbing; }
.zp__content { position: absolute; left: 0; top: 0; transform-origin: 0 0; width: max-content; }
.zp__content :deep(img), .zp__content :deep(svg) { display: block; max-width: none; user-select: none; -webkit-user-drag: none; }
</style>
