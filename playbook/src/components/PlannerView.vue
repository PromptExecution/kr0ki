<script setup>
// The diagram planner as its own page: a chat with the planning agent on the left, and on the right what it is
// currently recommending (the cards it highlights/selects through the UI bridge) with one-click Edit / Agent.
import { computed } from 'vue'
import StoryB00k from './StoryB00k.vue'

const props = defineProps({
  catalog: { type: Object, default: null },
  examples: { type: Array, default: () => [] },
  agentUrl: { type: String, default: '' },
  plannerSession: { type: String, default: '' },
  uiStatus: { type: String, default: 'closed' },
  suggested: { type: Array, default: () => [] },
  suggestNote: { type: String, default: '' },
  selectedTypeId: { type: String, default: '' },
})
const emit = defineEmits(['open-in-editor', 'agent-handoff', 'show-in-gallery'])

const byId = computed(() => new Map((props.catalog?.types || []).map((t) => [t.id, t])))
// The picks: the shortlist, plus the selected type if it is not already on it.
const picks = computed(() => {
  const ids = [...props.suggested]
  if (props.selectedTypeId && !ids.includes(props.selectedTypeId)) ids.push(props.selectedTypeId)
  return ids.map((id) => byId.value.get(id)).filter(Boolean).map((t) => ({
    ...t,
    example: props.examples.find((e) => e.id === t.exampleId),
    primary: t.id === props.selectedTypeId,
  }))
})

function edit(pick) {
  if (pick.example) emit('open-in-editor', pick.example)
}
function agent(pick) {
  emit('agent-handoff', { typeId: pick.id, syntax: pick.syntax, prompt: pick.samplePrompt })
}
</script>

<template>
  <section class="planner-view" data-testid="planner-view">
    <div class="planner-chat">
      <header class="planner-head">
        <h2>Diagram planner</h2>
        <span class="planner-link" :data-status="uiStatus" :title="`UI session ${plannerSession}`">
          {{ uiStatus === 'open' ? '● page linked' : uiStatus === 'connecting' ? '○ linking…' : '○ not linked' }}
        </span>
      </header>
      <p class="planner-intro">Tell me what you want to show. I'll look through the catalog and suggest the best-fit diagram type.</p>
      <StoryB00k v-if="plannerSession" planner :thread-id="plannerSession" :agent-url="agentUrl" />
      <p v-if="plannerSession" class="planner-session" data-testid="planner-session">
        MCP clients can steer this page with <code>navigate_ui</code>, session <code>{{ plannerSession }}</code>
      </p>
    </div>

    <aside class="planner-picks" aria-label="Planner's picks" data-testid="planner-picks">
      <h3>Planner's picks</h3>
      <p v-if="!picks.length" class="planner-empty">Nothing suggested yet. Describe what you want to show and the picks appear here.</p>
      <p v-else-if="suggestNote" class="planner-note">{{ suggestNote }}</p>
      <article v-for="pick in picks" :key="pick.id" class="pick-card" :class="{ primary: pick.primary }" :data-type-id="pick.id">
        <header>
          <strong>{{ pick.name }}</strong>
          <span class="pick-syntax">{{ pick.syntax }}</span>
          <span v-if="pick.primary" class="pick-best">★ best fit</span>
        </header>
        <p class="pick-blurb">{{ pick.blurb }}</p>
        <p class="pick-uses">{{ pick.useCases.join(' · ') }}</p>
        <footer>
          <button type="button" class="pick-btn" :disabled="!pick.example" data-testid="pick-edit" @click="edit(pick)">Edit</button>
          <button type="button" class="pick-btn agent" data-testid="pick-agent" @click="agent(pick)">Agent</button>
          <button type="button" class="pick-btn secondary" data-testid="pick-gallery" @click="emit('show-in-gallery', pick.id)">Show in gallery</button>
        </footer>
      </article>
    </aside>
  </section>
</template>

<style scoped>
.planner-view { display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(16rem, 1fr); gap: 1rem; align-items: start; }
.planner-chat, .planner-picks { min-width: 0; display: flex; flex-direction: column; gap: .5rem; }
.planner-head { display: flex; align-items: baseline; gap: .75rem; }
.planner-head h2 { margin: 0; font-size: 1.1rem; }
.planner-link { font-size: .72rem; font-weight: 700; color: #94a6c8; }
.planner-link[data-status="open"] { color: #4ade80; }
.planner-intro { margin: 0; color: #a7b5d4; font-size: .85rem; }
.planner-session { margin: 0; font-size: .7rem; color: #6f7fa3; overflow-wrap: anywhere; }
.planner-picks { position: sticky; top: 1rem; padding: .75rem; background: #101a36; border: 1px solid #2a3966; border-radius: .85rem; }
.planner-picks h3 { margin: 0; font-size: .95rem; }
.planner-empty { margin: 0; color: #6f7fa3; font-size: .85rem; }
.planner-note { margin: 0; padding: .45rem .6rem; background: #2a1f4d; border: 1px solid #a855f7; border-radius: .5rem; color: #e9d5ff; font-size: .82rem; }
.pick-card { padding: .6rem .7rem; background: #131d3c; border: 1px solid #2a3966; border-radius: .6rem; display: flex; flex-direction: column; gap: .3rem; }
.pick-card.primary { border-color: #38bdf8; box-shadow: 0 0 0 1px #38bdf8; }
.pick-card header { display: flex; align-items: baseline; gap: .5rem; flex-wrap: wrap; }
.pick-syntax { color: #6f7fa3; font-size: .7rem; }
.pick-best { margin-left: auto; color: #38bdf8; font-size: .7rem; font-weight: 800; }
.pick-blurb { margin: 0; color: #cdd8ee; font-size: .82rem; line-height: 1.35; }
.pick-uses { margin: 0; color: #7f8db2; font-size: .7rem; }
.pick-card footer { display: flex; gap: .4rem; flex-wrap: wrap; }
.pick-btn { border: 0; border-radius: .4rem; padding: .3rem .6rem; font-weight: 700; font-size: .75rem; cursor: pointer; background: #38bdf8; color: #052235; }
.pick-btn.agent { background: #a855f7; color: #fff; }
.pick-btn.secondary { background: transparent; color: #9cc9ff; border: 1px solid #3b4d7d; }
.pick-btn:disabled { opacity: .5; cursor: not-allowed; }
@media (max-width: 980px) { .planner-view { grid-template-columns: 1fr; } .planner-picks { position: static; } }
</style>
