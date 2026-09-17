<script setup>
import { ref } from 'vue'
import { useChat } from '@synoped/ag-ui-vue'
import StoryB00kPanel from './StoryB00kPanel.vue'

const agentUrl = import.meta.env.VITE_STORYB00K_AGENT_URL || 'http://localhost:8789'
const input = ref('')
const chat = useChat({ url: `${agentUrl}/run`, initialState: { panels: [] } })

async function sendMessage() {
  const text = input.value.trim()
  if (!text) return
  input.value = ''
  await chat.send(text)
}

async function answerInterrupt(interrupt, approved) {
  await fetch(`${agentUrl}/respond-to-interrupt`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ threadId: chat.threadId.value, interruptId: interrupt.id, approved }),
  })
  chat.respondToInterrupt(interrupt.id, { approved })
}
</script>

<template>
  <div class="storyb00k">
    <section class="storyb00k__transcript">
      <h2>storyb00k</h2>
      <p>Read the live model, assemble evidence panels, and narrate without changing the authoritative model.</p>
      <div v-for="item in chat.items" :key="item.id" class="storyb00k__message">
        <strong>{{ item.role }}:</strong>
        <span v-for="(part, index) in item.parts" :key="index">{{ part.type === 'text' ? part.text : '' }}</span>
      </div>
      <div v-for="interrupt in chat.interrupts" :key="interrupt.id" class="storyb00k__interrupt">
        <p>{{ interrupt.reason || interrupt.message }}</p>
        <button @click="answerInterrupt(interrupt, true)">Approve draft</button>
        <button @click="answerInterrupt(interrupt, false)">Decline</button>
      </div>
      <label>
        Ask about this system
        <input v-model="input" :disabled="chat.status !== 'ready'" @keyup.enter="sendMessage" />
      </label>
      <button :disabled="chat.status !== 'ready' || !input.trim()" @click="sendMessage">Send</button>
      <p v-if="chat.error" class="error">{{ chat.error.message }}</p>
    </section>
    <section class="storyb00k__dashboard" aria-label="Agent dashboard">
      <StoryB00kPanel v-for="(panel, index) in chat.state.panels || []" :key="index" :panel="panel" />
    </section>
  </div>
</template>

<style scoped>
.storyb00k { display: grid; gap: 1rem; grid-template-columns: minmax(18rem, .8fr) minmax(22rem, 1.2fr); }
.storyb00k__transcript, .storyb00k__dashboard { min-width: 0; }
.storyb00k__message { margin: .5rem 0; }
.storyb00k__interrupt { border-left: 3px solid #b57700; padding-left: .75rem; }
@media (max-width: 760px) { .storyb00k { grid-template-columns: 1fr; } }
</style>
