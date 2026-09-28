<script setup>
import { ref, onMounted } from 'vue'

const props = defineProps({
  rendererUrl: { type: String, default: '' },
  agentUrl: { type: String, default: '' },
})

const emit = defineEmits(['update:rendererUrl', 'update:llmUrl', 'update:llmKey', 'update:llmModel', 'update:agentUrl'])

// Local state for form inputs
const localRendererUrl = ref(props.rendererUrl)
const localAgentUrl = ref(props.agentUrl)
const llmUrl = ref('')
const llmKey = ref('')
const llmModel = ref('')
const showKey = ref(false)
const saved = ref(false)

// Test state
const rendererTestStatus = ref('') // '', 'testing', 'success', 'error'
const rendererTestMessage = ref('')
const llmTestStatus = ref('')
const llmTestMessage = ref('')
const agentTestStatus = ref('')
const agentTestMessage = ref('')
const availableModels = ref([])

// Computed for template access to window
const defaultLlmPlaceholder = ref('')

// Load from localStorage on mount
onMounted(() => {
  // Use the current hostname instead of localhost for remote access
  const hostname = window.location.hostname
  const defaultLlmUrl = `http://${hostname}:8002/v1`
  const defaultAgentUrl = `http://${hostname}:8789`
  defaultLlmPlaceholder.value = defaultLlmUrl
  llmUrl.value = localStorage.getItem('kr0ki:llmUrl') || defaultLlmUrl
  llmKey.value = localStorage.getItem('kr0ki:llmKey') || ''
  llmModel.value = localStorage.getItem('kr0ki:llmModel') || 'gpt-4o'
  localAgentUrl.value = localStorage.getItem('kr0ki:agentUrl') || defaultAgentUrl
})

// Watch for prop changes
import { watch } from 'vue'
watch(() => props.rendererUrl, (newVal) => {
  localRendererUrl.value = newVal
})

function saveSettings() {
  // Save LLM settings to localStorage
  localStorage.setItem('kr0ki:llmUrl', llmUrl.value)
  localStorage.setItem('kr0ki:llmKey', llmKey.value)
  localStorage.setItem('kr0ki:llmModel', llmModel.value)
  localStorage.setItem('kr0ki:agentUrl', localAgentUrl.value)
  
  // Emit updates
  emit('update:rendererUrl', localRendererUrl.value)
  emit('update:llmUrl', llmUrl.value)
  emit('update:llmKey', llmKey.value)
  emit('update:llmModel', llmModel.value)
  emit('update:agentUrl', localAgentUrl.value)
  
  // Show saved indicator
  saved.value = true
  setTimeout(() => { saved.value = false }, 2000)
}

function clearLlmSettings() {
  const defaultLlmUrl = `http://${window.location.hostname}:8002/v1`
  llmUrl.value = defaultLlmUrl
  llmKey.value = ''
  llmModel.value = 'gpt-4o'
  localStorage.removeItem('kr0ki:llmUrl')
  localStorage.removeItem('kr0ki:llmKey')
  localStorage.removeItem('kr0ki:llmModel')
  emit('update:llmUrl', defaultLlmUrl)
  emit('update:llmKey', '')
  emit('update:llmModel', 'gpt-4o')
}

async function testRenderer() {
  rendererTestStatus.value = 'testing'
  rendererTestMessage.value = ''
  
  try {
    const url = localRendererUrl.value.replace(/\/$/, '')
    const response = await fetch(`${url}/health`, {
      method: 'GET',
      headers: { 'Accept': 'application/json' }
    })
    
    if (response.ok) {
      const data = await response.json()
      if (data.status === 'ok' || data.status === 'degraded') {
        rendererTestStatus.value = 'success'
        rendererTestMessage.value = `✓ Renderer operational (status: ${data.status})`
      } else {
        rendererTestStatus.value = 'error'
        rendererTestMessage.value = `✗ Unexpected status: ${data.status}`
      }
    } else {
      rendererTestStatus.value = 'error'
      rendererTestMessage.value = `✗ HTTP ${response.status}: ${response.statusText}`
    }
  } catch (error) {
    rendererTestStatus.value = 'error'
    rendererTestMessage.value = `✗ Connection failed: ${error.message}`
  }
}

async function testLlm() {
  llmTestStatus.value = 'testing'
  llmTestMessage.value = ''
  
  if (!llmUrl.value) {
    llmTestStatus.value = 'error'
    llmTestMessage.value = '✗ Please configure the API URL'
    return
  }
  
  try {
    const url = llmUrl.value.replace(/\/$/, '')
    const headers = { 'Accept': 'application/json' }
    
    // Add authorization header only if API key is provided
    if (llmKey.value) {
      headers['Authorization'] = `Bearer ${llmKey.value}`
    }
    
    const response = await fetch(`${url}/models`, {
      method: 'GET',
      headers
    })
    
    if (response.ok) {
      const data = await response.json()
      const models = data.data || []
      availableModels.value = models.map(m => ({
        id: m.id,
        name: m.id
      }))
      const modelCount = models.length
      const modelName = models[0]?.id || 'unknown'
      llmTestStatus.value = 'success'
      llmTestMessage.value = `✓ LLM API operational (${modelCount} model${modelCount !== 1 ? 's' : ''} available: ${modelName})`
      
      // Auto-select first model if current selection is not in the list
      if (modelCount > 0 && !availableModels.value.some(m => m.id === llmModel.value)) {
        llmModel.value = models[0].id
      }
    } else if (response.status === 401) {
      llmTestStatus.value = 'error'
      llmTestMessage.value = '✗ Authentication failed: Invalid API key'
    } else {
      llmTestStatus.value = 'error'
      llmTestMessage.value = `✗ HTTP ${response.status}: ${response.statusText}`
    }
  } catch (error) {
    llmTestStatus.value = 'error'
    llmTestMessage.value = `✗ Connection failed: ${error.message}`
  }
}

async function testAgent() {
  agentTestStatus.value = 'testing'
  agentTestMessage.value = ''
  
  try {
    const url = localAgentUrl.value.replace(/\/$/, '')
    const controller = new AbortController()
    const timeoutId = setTimeout(() => controller.abort(), 5000)
    
    const response = await fetch(`${url}/health`, {
      method: 'GET',
      headers: { 'Accept': 'application/json' },
      signal: controller.signal,
    })
    clearTimeout(timeoutId)
    
    if (response.ok) {
      const data = await response.json()
      agentTestStatus.value = 'success'
      agentTestMessage.value = `✓ Agent operational (status: ${data.status || 'ok'})`
    } else {
      agentTestStatus.value = 'error'
      agentTestMessage.value = `✗ HTTP ${response.status}: ${response.statusText}`
    }
  } catch (error) {
    agentTestStatus.value = 'error'
    if (error.name === 'AbortError') {
      agentTestMessage.value = '✗ Connection timeout (5s) - agent server not responding'
    } else if (error.message.includes('Failed to fetch')) {
      agentTestMessage.value = `✗ Cannot reach agent at ${localAgentUrl.value} - is it running?`
    } else {
      agentTestMessage.value = `✗ Connection failed: ${error.message}`
    }
  }
}
</script>

<template>
  <div class="setup">
    <header class="setup__header">
      <h2>Setup</h2>
      <p class="setup__description">Configure service endpoints and credentials.</p>
    </header>

    <form class="setup__form" @submit.prevent="saveSettings">
      <section class="setup__section">
        <h3>Renderer</h3>
        <label class="setup__field">
          <span class="setup__label">Renderer URL</span>
          <input
            v-model="localRendererUrl"
            type="url"
            placeholder="http://127.0.0.1:8787"
            class="setup__input"
          />
          <span class="setup__hint">The kr0ki server that renders diagrams. Defaults to the current host.</span>
        </label>
        <div class="setup__actions">
          <button type="button" class="setup__btn setup__btn--secondary" @click="testRenderer" :disabled="rendererTestStatus === 'testing'">
            {{ rendererTestStatus === 'testing' ? 'Testing...' : 'Test Connection' }}
          </button>
          <span v-if="rendererTestMessage" :class="['setup__test-result', rendererTestStatus]">
            {{ rendererTestMessage }}
          </span>
        </div>
      </section>

      <section class="setup__section">
        <h3>Agent</h3>
        <p class="setup__section-desc">The storyb00k agent server that orchestrates diagram generation.</p>
        
        <label class="setup__field">
          <span class="setup__label">Agent URL</span>
          <input
            v-model="localAgentUrl"
            type="url"
            placeholder="http://127.0.0.1:8789"
            class="setup__input"
          />
          <span class="setup__hint">The storyb00k agent server. Defaults to the current host on port 8789.</span>
        </label>
        <div class="setup__actions">
          <button type="button" class="setup__btn setup__btn--secondary" @click="testAgent" :disabled="agentTestStatus === 'testing'">
            {{ agentTestStatus === 'testing' ? 'Testing...' : 'Test Connection' }}
          </button>
          <span v-if="agentTestMessage" :class="['setup__test-result', agentTestStatus]">
            {{ agentTestMessage }}
          </span>
        </div>
      </section>

      <section class="setup__section">
        <h3>LLM Configuration</h3>
        <p class="setup__section-desc">Used by the storyb00k agent for diagram generation.</p>
        
        <label class="setup__field">
          <span class="setup__label">LLM API URL</span>
          <input
            v-model="llmUrl"
            type="url"
            :placeholder="defaultLlmPlaceholder"
            class="setup__input"
          />
          <span class="setup__hint">OpenAI-compatible API endpoint. Use the server's hostname/IP (not localhost) for remote access.</span>
        </label>

        <label class="setup__field">
          <span class="setup__label">API Key</span>
          <div class="setup__input-group">
            <input
              v-model="llmKey"
              :type="showKey ? 'text' : 'password'"
              placeholder="sk-..."
              class="setup__input setup__input--key"
            />
            <button
              type="button"
              class="setup__toggle"
              @click="showKey = !showKey"
              :title="showKey ? 'Hide API key' : 'Show API key'"
            >
              {{ showKey ? '🙈' : '👁️' }}
            </button>
          </div>
          <span class="setup__hint">Stored locally in your browser. Never sent to the kr0ki server.</span>
        </label>

        <label class="setup__field">
          <span class="setup__label">Model</span>
          <input
            v-model="llmModel"
            list="available-models"
            type="text"
            placeholder="Test connection to load models, or enter manually"
            class="setup__input"
          />
          <datalist id="available-models">
            <option v-for="model in availableModels" :key="model.id" :value="model.id">
              {{ model.name }}
            </option>
          </datalist>
          <span class="setup__hint">The model used for diagram generation. Models are loaded from the LLM server when you test the connection.</span>
        </label>

        <div class="setup__actions">
          <button type="button" class="setup__btn setup__btn--secondary" @click="testLlm" :disabled="llmTestStatus === 'testing'">
            {{ llmTestStatus === 'testing' ? 'Testing...' : 'Test Connection' }}
          </button>
          <button type="button" class="setup__btn setup__btn--secondary" @click="clearLlmSettings">
            Clear LLM Settings
          </button>
          <span v-if="llmTestMessage" :class="['setup__test-result', llmTestStatus]">
            {{ llmTestMessage }}
          </span>
        </div>
      </section>

      <div class="setup__footer">
        <button type="submit" class="setup__btn setup__btn--primary">
          {{ saved ? '✓ Saved' : 'Save Settings' }}
        </button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.setup {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  max-width: 40rem;
}

.setup__header h2 {
  margin: 0 0 0.25rem;
  font-size: 1.5rem;
}

.setup__description {
  margin: 0;
  color: #b9c6dd;
  font-size: 0.95rem;
}

.setup__form {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.setup__section {
  background: #131d3c;
  border: 1px solid #2a3966;
  border-radius: 0.75rem;
  padding: 1.25rem;
}

.setup__section h3 {
  margin: 0 0 0.5rem;
  font-size: 1.1rem;
  color: #7dd3fc;
}

.setup__section-desc {
  margin: 0 0 1rem;
  color: #94a6c8;
  font-size: 0.85rem;
}

.setup__field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-bottom: 1rem;
}

.setup__field:last-child {
  margin-bottom: 0;
}

.setup__label {
  font-weight: 600;
  font-size: 0.9rem;
  color: #ced8ee;
}

.setup__input {
  width: 100%;
  border: 1px solid #3b4d7d;
  border-radius: 0.45rem;
  background: #091127;
  color: #edf5ff;
  padding: 0.55rem 0.65rem;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.85rem;
}

.setup__input:focus {
  outline: none;
  border-color: #38bdf8;
  box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.2);
}

.setup__input--key {
  font-family: inherit;
}

.setup__input-group {
  display: flex;
  gap: 0.5rem;
}

.setup__input-group .setup__input {
  flex: 1;
}

.setup__toggle {
  border: 1px solid #3b4d7d;
  border-radius: 0.45rem;
  background: #091127;
  color: #edf5ff;
  padding: 0.55rem 0.75rem;
  cursor: pointer;
  font-size: 1rem;
  transition: background 0.15s ease;
}

.setup__toggle:hover {
  background: #1b2a52;
}

.setup__hint {
  font-size: 0.78rem;
  color: #7f8db2;
  line-height: 1.4;
}

.setup__actions {
  margin-top: 1rem;
  display: flex;
  gap: 0.5rem;
}

.setup__footer {
  display: flex;
  justify-content: flex-end;
}

.setup__btn {
  border: 0;
  border-radius: 0.45rem;
  padding: 0.65rem 1.25rem;
  font-weight: 700;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.15s ease;
}

.setup__btn--primary {
  background: #38bdf8;
  color: #052235;
}

.setup__btn--primary:hover {
  filter: brightness(1.1);
}

.setup__btn--secondary {
  background: transparent;
  color: #9cc9ff;
  border: 1px solid #3b4d7d;
}

.setup__btn--secondary:hover {
  background: #1b2a52;
}

.setup__test-result {
  font-size: 0.85rem;
  padding: 0.4rem 0.8rem;
  border-radius: 0.4rem;
  font-weight: 600;
}

.setup__test-result.success {
  background: rgba(34, 197, 94, 0.15);
  color: #4ade80;
  border: 1px solid rgba(34, 197, 94, 0.3);
}

.setup__test-result.error {
  background: rgba(239, 68, 68, 0.15);
  color: #f87171;
  border: 1px solid rgba(239, 68, 68, 0.3);
}
</style>
