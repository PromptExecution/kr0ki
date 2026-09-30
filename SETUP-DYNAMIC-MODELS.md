# Setup Component - Dynamic Model Loading

## Overview
Updated the Setup component to dynamically load available models from the LLM server instead of using a hardcoded list. This provides flexibility and ensures the UI always reflects the actual models available on the configured LLM server.

## Changes Made

### 1. Removed Hardcoded Model List
- **Before**: Static `modelOptions` array with predefined models for OpenAI, Anthropic, Google, etc.
- **After**: Dynamic `availableModels` array populated from the server's `/models` endpoint

### 2. Dynamic Model Fetching
When the user clicks "Test Connection" for the LLM:
1. The component calls the `/models` endpoint
2. Parses the response and extracts model IDs
3. Populates the `availableModels` array
4. Auto-selects the first model if the current selection isn't in the list
5. Displays the count and first model name in the success message

### 3. UI Changes
Changed the model selection from a `<select>` dropdown to a text `<input>` with a `<datalist>`:
- **Benefits**:
  - Works before testing the connection (not disabled)
  - Allows manual input of custom model names
  - Provides autocomplete suggestions from fetched models
  - Preserves saved model values even if not in the current list
  - More flexible for different LLM providers

### 4. Default Values
- **LLM URL**: Defaults to `http://localhost:8002/v1` (llama-cpp server)
- **LLM Key**: No default (optional for local servers)
- **Model**: No default (loaded from server or entered manually)

## Implementation Details

### State Management
```javascript
const availableModels = ref([])
```

### Model Fetching Logic
```javascript
if (response.ok) {
  const data = await response.json()
  const models = data.data || []
  availableModels.value = models.map(m => ({
    id: m.id,
    name: m.id
  }))
  
  // Auto-select first model if current selection is not in the list
  if (modelCount > 0 && !availableModels.value.some(m => m.id === llmModel.value)) {
    llmModel.value = models[0].id
  }
}
```

### Template Structure
```vue
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
```

## User Experience

### Before Testing Connection
- Model field shows placeholder text: "Test connection to load models, or enter manually"
- User can manually type a model name
- If a model was previously saved, it's displayed

### After Testing Connection
- Model field shows autocomplete suggestions from the server
- User can select from the list or type a custom name
- Success message shows model count and first model name
- Example: "✓ LLM API operational (3 models available: llama-3.2-3b)"

### Clearing Settings
- Clicking "Clear LLM Settings" resets:
  - URL to `http://localhost:8002/v1`
  - Key to empty
  - Model to empty
  - `availableModels` array is NOT cleared (still shows last fetched models)

## Benefits

1. **Flexibility**: Works with any OpenAI-compatible LLM server
2. **Accuracy**: Always shows actual available models
3. **User-Friendly**: Autocomplete helps users select valid models
4. **No Maintenance**: No need to update hardcoded lists when models change
5. **Backward Compatible**: Saved model values are preserved

## Testing the Feature

1. Navigate to http://127.0.0.1:8787/playbook/
2. Click "Setup" in the navigation
3. Verify LLM URL defaults to `http://localhost:8002/v1`
4. Click "Test Connection" for LLM
5. Observe the success message showing model count
6. Click the Model field and see autocomplete suggestions
7. Select a model or type a custom name
8. Click "Save Settings"
9. Refresh the page and verify the model is still selected

## Future Enhancements

- Add model metadata display (context window, capabilities)
- Group models by provider/type
- Add model search/filter for large lists
- Cache model list with timestamp
- Show model details on hover
