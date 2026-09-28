# Setup Panel Implementation

## Overview
Added a "Setup" button to the playbook navigation that provides a configuration interface for service endpoints and credentials.

## Changes Made

### 1. New Component: `Setup.vue`
Created `/playbook/src/components/Setup.vue` with:
- **Renderer URL configuration**: Configure the kr0ki server endpoint
- **LLM Configuration section**:
  - LLM API URL (OpenAI-compatible endpoint)
  - API Key (with show/hide toggle)
- **LocalStorage persistence**: Settings persist across browser sessions
- **Clear LLM Settings button**: Remove stored credentials
- **Visual feedback**: "✓ Saved" indicator after saving

### 2. Updated `App.vue`
- Imported `Setup` component
- Added `viewMode === 'setup'` state
- Added "Setup" button to navigation tabs (after Gallery, Editor, Agent)
- Added settings state management:
  - `rendererUrl`: Renderer endpoint (persisted to localStorage)
  - `llmUrl`: LLM API URL (persisted to localStorage)
  - `llmKey`: LLM API key (persisted to localStorage)
- Added update handlers for each setting
- Passed `rendererUrl` prop to `RendererPanel`
- Passed `llmUrl` and `llmKey` props to `StoryB00k` (for future use)

### 3. Updated `RendererPanel.vue`
- Added `rendererUrl` prop to receive URL from Setup
- Renamed internal `rendererUrl` ref to `localRendererUrl` to avoid conflict
- Added watcher to sync prop changes from Setup
- Falls back to query parameter or current origin if prop not provided

### 4. Navigation Layout
```
┌─────────────────────────────────────┐
│  Gallery  Editor  Agent  Setup     │
└─────────────────────────────────────┘
```

## Features

### Renderer URL
- Configurable kr0ki server endpoint
- Persisted to `localStorage` key `kr0ki:rendererUrl`
- Automatically passed to RendererPanel for all renders
- Falls back to current host if not set

### LLM Configuration
- **API URL**: OpenAI-compatible endpoint (e.g., `https://api.openai.com/v1`)
- **API Key**: Secure password field with show/hide toggle
- Both persisted to localStorage:
  - `kr0ki:llmUrl`
  - `kr0ki:llmKey`
- "Clear LLM Settings" button removes both from storage
- Security note: Keys stored locally in browser, never sent to kr0ki server

## Usage

1. Click "Setup" in the navigation
2. Configure the Renderer URL (defaults to current host)
3. Optionally configure LLM settings for the storyb00k agent
4. Click "Save Settings"
5. Settings persist across browser sessions

## Testing

Build the playbook:
```bash
cd playbook
npm run build
```

Start the server:
```bash
just start
```

Navigate to http://127.0.0.1:8787/playbook/ and click "Setup" in the navigation.

## Storage Keys

- `kr0ki:rendererUrl` - Renderer endpoint
- `kr0ki:llmUrl` - LLM API URL
- `kr0ki:llmKey` - LLM API key

## Future Enhancements

- LLM settings could be passed to the storyb00k agent via configuration endpoint
- Add connection test buttons for Renderer and LLM endpoints
- Add more service configurations (e.g., SysML v2 server, KubeDiagrams worker)
- Export/import settings for backup/migration
