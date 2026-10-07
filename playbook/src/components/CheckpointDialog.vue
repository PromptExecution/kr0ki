<script setup>
// "Save to project": commit a diagram into one of the local projects (or a new one) as a checkpoint, so it can be recovered later
// from Projects -> History. The store is opened only when the person acts, so rendering this panel never touches storage.
import { computed, ref } from 'vue'
import { LocalStorage } from 'quasar'
import { createProjectStore } from '../lib/projects.js'
import { checkpointPath, saveCheckpoint } from '../lib/checkpoint.js'

const props = defineProps({
  source: { type: String, default: '' },
  format: { type: String, default: 'd2' },
  title: { type: String, default: 'diagram' },
  /** injected in tests */
  store: { type: Object, default: null },
})
const emit = defineEmits(['saved'])

let store = props.store
const open = ref(false)
const projects = ref([])
const choice = ref('__new__')
const newName = ref('')
const path = ref('')
const message = ref('')
const notice = ref(null)

function ensureStore() {
  store = store || createProjectStore({ storage: LocalStorage })
  return store
}
function toggle() {
  open.value = !open.value
  if (!open.value) return
  projects.value = ensureStore().listProjects()
  choice.value = projects.value[0]?.id || '__new__'
  newName.value = newName.value || props.title || 'Diagrams'
  path.value = checkpointPath(props.format, props.title)
  message.value = `Checkpoint: ${props.title}`
  notice.value = null
}
const creating = computed(() => choice.value === '__new__')

function save() {
  try {
    const r = saveCheckpoint(ensureStore(), {
      projectId: creating.value ? '' : choice.value,
      newProjectName: newName.value,
      path: path.value.trim(),
      source: props.source,
      message: message.value.trim(),
    })
    notice.value = r.unchanged
      ? { kind: 'ok', text: `Nothing to save: ${path.value} already has exactly this code in "${r.project.name}".` }
      : { kind: 'ok', text: `Saved checkpoint ${r.commit.id.slice(0, 8)} to "${r.project.name}". Recover it in Projects → History → Restore.` }
    projects.value = ensureStore().listProjects()
    if (!r.unchanged) emit('saved', { project: r.project, commit: r.commit, path: path.value })
  } catch (err) {
    notice.value = { kind: 'error', text: err.message }
  }
}
</script>

<template>
  <div class="cp" data-testid="checkpoint">
    <button type="button" data-testid="cp-toggle" :disabled="!source.trim()" title="Commit this diagram's code to a project so you can come back to it" @click="toggle">📌 Save to project…</button>
    <form v-if="open" class="cp__form" @submit.prevent="save">
      <label>Project
        <select v-model="choice" data-testid="cp-project">
          <option v-for="p in projects" :key="p.id" :value="p.id">{{ p.name }}</option>
          <option value="__new__">＋ New project…</option>
        </select>
      </label>
      <label v-if="creating">New project name <input v-model="newName" data-testid="cp-name" maxlength="80" /></label>
      <label>File <input v-model="path" data-testid="cp-path" spellcheck="false" /></label>
      <label>Note <input v-model="message" data-testid="cp-message" /></label>
      <button type="submit" data-testid="cp-save">Save checkpoint</button>
      <p v-if="notice" class="cp__notice" :data-kind="notice.kind" data-testid="cp-notice" role="status">{{ notice.text }}</p>
    </form>
  </div>
</template>

<style scoped>
.cp { display: flex; flex-direction: column; gap: .4rem; align-items: flex-start; }
.cp__form { display: flex; flex-direction: column; gap: .35rem; padding: .5rem .6rem; border: 1px solid #c8ced8; border-radius: 6px; font-size: .82rem; width: min(100%, 28rem); box-sizing: border-box; }
.cp__form label { display: flex; flex-direction: column; gap: .1rem; }
.cp__form input, .cp__form select { font: inherit; padding: .15rem .3rem; }
.cp__notice { margin: 0; }
.cp__notice[data-kind='ok'] { color: #15803d; }
.cp__notice[data-kind='error'] { color: #b91c1c; }
</style>
