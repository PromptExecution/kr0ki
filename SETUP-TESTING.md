# Setup Panel - Connection Testing Feature

## Overview
Added "Test Connection" buttons to both the Renderer and LLM configuration sections in the Setup panel. These buttons validate that the configured endpoints are operational before saving settings.

## Features

### Renderer Test
- **Endpoint**: `{rendererUrl}/health`
- **Method**: GET
- **Validation**: Checks if the kr0ki server responds with a valid health status
- **Success Criteria**: HTTP 200 with `status` field equal to "ok" or "degraded"
- **Error Handling**: Displays HTTP status codes and connection errors

### LLM Test
- **Endpoint**: `{llmUrl}/models`
- **Method**: GET with Bearer token authentication
- **Validation**: Checks if the LLM API responds and lists available models
- **Success Criteria**: HTTP 200 with valid JSON response containing model list
- **Error Handling**: 
  - Validates that both API URL and API Key are configured
  - Detects 401 authentication failures
  - Displays HTTP status codes and connection errors

## Implementation Details

### State Management
```javascript
const rendererTestStatus = ref('') // '', 'testing', 'success', 'error'
const rendererTestMessage = ref('')
const llmTestStatus = ref('')
const llmTestMessage = ref('')
```

### Test Functions

#### testRenderer()
1. Sets status to 'testing'
2. Makes GET request to `{rendererUrl}/health`
3. Parses JSON response
4. Validates status field
5. Updates status and message based on result
6. Handles network errors gracefully

#### testLlm()
1. Validates that both URL and Key are configured
2. Sets status to 'testing'
3. Makes GET request to `{llmUrl}/models` with Bearer token
4. Parses JSON response
5. Counts available models
6. Updates status and message based on result
7. Handles authentication and network errors

### UI Components

#### Renderer Section
```html
<div class="setup__actions">
  <button @click="testRenderer" :disabled="rendererTestStatus === 'testing'">
    {{ rendererTestStatus === 'testing' ? 'Testing...' : 'Test Connection' }}
  </button>
  <span v-if="rendererTestMessage" :class="['setup__test-result', rendererTestStatus]">
    {{ rendererTestMessage }}
  </span>
</div>
```

#### LLM Section
```html
<div class="setup__actions">
  <button @click="testLlm" :disabled="llmTestStatus === 'testing'">
    {{ llmTestStatus === 'testing' ? 'Testing...' : 'Test Connection' }}
  </button>
  <button @click="clearLlmSettings">Clear LLM Settings</button>
  <span v-if="llmTestMessage" :class="['setup__test-result', llmTestStatus]">
    {{ llmTestMessage }}
  </span>
</div>
```

### Styling

#### Success State
- Background: `rgba(34, 197, 94, 0.15)` (green tint)
- Text: `#4ade80` (bright green)
- Border: `rgba(34, 197, 94, 0.3)`

#### Error State
- Background: `rgba(239, 68, 68, 0.15)` (red tint)
- Text: `#f87171` (bright red)
- Border: `rgba(239, 68, 68, 0.3)`

## User Experience

### Testing Flow
1. User configures endpoint URL (and API key for LLM)
2. User clicks "Test Connection" button
3. Button text changes to "Testing..." and becomes disabled
4. Test executes asynchronously
5. Result message appears with color-coded status
6. User can retry if test fails

### Validation Messages

#### Renderer Success
```
✓ Renderer operational (status: ok)
```

#### Renderer Error
```
✗ HTTP 503: Service Unavailable
✗ Connection failed: Failed to fetch
```

#### LLM Success
```
✓ LLM API operational (42 models available)
```

#### LLM Errors
```
✗ Please configure both API URL and API Key
✗ Authentication failed: Invalid API key
✗ HTTP 404: Not Found
✗ Connection failed: Network error
```

## Security Considerations

1. **No Credential Storage**: Test results don't persist credentials
2. **Client-Side Only**: Tests run in browser, no server-side proxy
3. **CORS Compliance**: Tests respect CORS policies of target endpoints
4. **Error Messages**: Don't expose sensitive information beyond HTTP status

## Testing the Feature

1. Navigate to http://127.0.0.1:8787/playbook/
2. Click "Setup" in the navigation
3. Configure Renderer URL (default: http://127.0.0.1:8787)
4. Click "Test Connection" for Renderer
5. Configure LLM API URL and API Key
6. Click "Test Connection" for LLM
7. Observe status messages and button states

## Future Enhancements

- Add timeout configuration for slow networks
- Cache test results with timestamps
- Add detailed error information (expandable)
- Test specific model availability for LLM
- Add latency measurement to test results
