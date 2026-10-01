#!/usr/bin/env node
/**
 * Functional Test Runner for kr0ki
 * 
 * This script runs functional tests using Chrome DevTools MCP to validate
 * user flows through actual browser interactions.
 * 
 * Usage: node test-runner.js [test-suite]
 * 
 * Test suites: core, agent, ui, all (default)
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Machine-local URLs come from the gitignored repo-root .env (see .env.example);
// BASE_URL / AGENT_URL in the environment still override. A missing .env is fine.
try {
  process.loadEnvFile(path.join(__dirname, '..', '..', '.env'));
} catch (err) {
  if (err.code !== 'ENOENT') throw err;
}

// Test configuration
const CONFIG = {
  baseUrl: process.env.BASE_URL || process.env.KR0KI_PUBLIC_URL || 'http://127.0.0.1:8787',
  agentUrl:
    process.env.AGENT_URL || process.env.KR0KI_AGENT_PUBLIC_URL || 'http://127.0.0.1:8789',
  timeout: 30000,
  screenshotDir: '/tmp/kr0ki-test-screenshots',
};

// Ensure screenshot directory exists
if (!fs.existsSync(CONFIG.screenshotDir)) {
  fs.mkdirSync(CONFIG.screenshotDir, { recursive: true });
}

// Test result tracking
const results = {
  passed: [],
  failed: [],
  skipped: [],
};

/**
 * Execute a Chrome DevTools MCP command
 */
function chromeCommand(action, params = {}) {
  try {
    const cmd = `mcp chrome-devtools-mcp ${action} '${JSON.stringify(params)}'`;
    const result = execSync(cmd, { encoding: 'utf-8', timeout: CONFIG.timeout });
    return JSON.parse(result);
  } catch (error) {
    console.error(`Chrome command failed: ${action}`, error.message);
    return null;
  }
}

/**
 * Take a screenshot and save it
 */
function takeScreenshot(name) {
  const filename = `${CONFIG.screenshotDir}/${name}-${Date.now()}.png`;
  chromeCommand('screenshot', { path: filename });
  return filename;
}

/**
 * Wait for an element to be present
 */
async function waitForElement(selector, timeout = 5000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    const result = chromeCommand('query', { selector });
    if (result && result.nodeId) {
      return result;
    }
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  return null;
}

/**
 * Click an element
 */
function clickElement(selector) {
  const element = chromeCommand('query', { selector });
  if (element && element.nodeId) {
    chromeCommand('click', { nodeId: element.nodeId });
    return true;
  }
  return false;
}

/**
 * Type text into an element
 */
function typeText(selector, text) {
  const element = chromeCommand('query', { selector });
  if (element && element.nodeId) {
    chromeCommand('type', { nodeId: element.nodeId, text });
    return true;
  }
  return false;
}

/**
 * Get text content of an element
 */
function getElementText(selector) {
  const result = chromeCommand('getAttributes', { selector });
  return result ? result.textContent : null;
}

/**
 * Run a single test
 */
async function runTest(test) {
  console.log(`\n🧪 Running: ${test.name}`);
  console.log(`   ${test.description}`);
  
  try {
    for (const step of test.steps) {
      console.log(`   → ${step.action}: ${step.description || ''}`);
      
      switch (step.action) {
        case 'navigate':
          chromeCommand('navigate', { url: step.url });
          await new Promise(resolve => setTimeout(resolve, 1000));
          break;
          
        case 'click':
          if (!clickElement(step.selector)) {
            throw new Error(`Element not found: ${step.selector}`);
          }
          await new Promise(resolve => setTimeout(resolve, 500));
          break;
          
        case 'type':
          if (!typeText(step.selector, step.text)) {
            throw new Error(`Element not found: ${step.selector}`);
          }
          await new Promise(resolve => setTimeout(resolve, 300));
          break;
          
        case 'wait':
          if (step.condition === 'element') {
            const element = await waitForElement(step.selector, step.timeout || 5000);
            if (!element) {
              throw new Error(`Element not found: ${step.selector}`);
            }
          }
          break;
          
        case 'assert':
          if (step.type === 'text') {
            const text = getElementText(step.selector);
            if (!text || !text.includes(step.expected)) {
              throw new Error(`Expected text "${step.expected}" not found in ${step.selector}`);
            }
          } else if (step.type === 'visible') {
            const element = await waitForElement(step.selector, 2000);
            if (!element) {
              throw new Error(`Element not visible: ${step.selector}`);
            }
          }
          break;
          
        case 'screenshot':
          const screenshot = takeScreenshot(step.name || test.name);
          console.log(`   📸 Screenshot: ${screenshot}`);
          break;
      }
    }
    
    console.log(`   ✅ PASSED`);
    results.passed.push(test.name);
    return true;
    
  } catch (error) {
    console.log(`   ❌ FAILED: ${error.message}`);
    takeScreenshot(`${test.name}-failed`);
    results.failed.push({ name: test.name, error: error.message });
    return false;
  }
}

// ============================================================================
// TEST DEFINITIONS
// ============================================================================

const CORE_TESTS = [
  {
    name: 'Core-001: kr0ki server health check',
    description: 'Verify kr0ki server is running and healthy',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/health`, description: 'Navigate to health endpoint' },
      { action: 'wait', condition: 'element', selector: 'body', timeout: 5000, description: 'Wait for response' },
      { action: 'assert', type: 'text', selector: 'body', expected: '"status":"ok"', description: 'Verify status is ok' },
    ],
  },
  {
    name: 'Core-002: Agent server health check',
    description: 'Verify agent server is running and healthy',
    steps: [
      { action: 'navigate', url: `${CONFIG.agentUrl}/health`, description: 'Navigate to agent health endpoint' },
      { action: 'wait', condition: 'element', selector: 'body', timeout: 5000, description: 'Wait for response' },
      { action: 'assert', type: 'text', selector: 'body', expected: '"status":"ok"', description: 'Verify status is ok' },
    ],
  },
  {
    name: 'Core-003: Direct diagram render (D2)',
    description: 'Verify direct API render works',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'wait', condition: 'element', selector: '.view-tab', description: 'Wait for UI to load' },
      { action: 'click', selector: '.view-tab:nth-child(2)', description: 'Click Code Editor tab' },
      { action: 'wait', condition: 'element', selector: 'textarea', description: 'Wait for editor' },
      { action: 'type', selector: 'textarea', text: 'x -> y -> z', description: 'Enter D2 source' },
      { action: 'click', selector: 'button:has-text("Render")', description: 'Click Render button' },
      { action: 'wait', condition: 'element', selector: 'img', timeout: 10000, description: 'Wait for rendered image' },
      { action: 'assert', type: 'visible', selector: 'img', description: 'Verify image is visible' },
      { action: 'screenshot', name: 'core-003-d2-render', description: 'Take screenshot' },
    ],
  },
  {
    name: 'Core-004: Auto-render on source change',
    description: 'Verify auto-render triggers on source change',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'click', selector: '.view-tab:nth-child(2)', description: 'Click Code Editor tab' },
      { action: 'wait', condition: 'element', selector: 'textarea', description: 'Wait for editor' },
      { action: 'type', selector: 'textarea', text: 'a -> b', description: 'Enter initial source' },
      { action: 'wait', condition: 'element', selector: 'img', timeout: 5000, description: 'Wait for auto-render' },
      { action: 'screenshot', name: 'core-004-auto-render-1', description: 'Screenshot after first render' },
      { action: 'type', selector: 'textarea', text: '\nc -> d', description: 'Modify source' },
      { action: 'wait', condition: 'element', selector: 'img', timeout: 5000, description: 'Wait for re-render' },
      { action: 'screenshot', name: 'core-004-auto-render-2', description: 'Screenshot after re-render' },
    ],
  },
];

const AGENT_TESTS = [
  {
    name: 'Agent-001: Direct agent request',
    description: 'Verify agent can generate diagram from text request',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'click', selector: '.view-tab:nth-child(3)', description: 'Click Agent tab' },
      { action: 'wait', condition: 'element', selector: 'textarea', description: 'Wait for input' },
      { action: 'type', selector: 'textarea', text: 'Create a simple flowchart with 3 boxes: Start, Process, End', description: 'Enter request' },
      { action: 'click', selector: 'button:has-text("Send")', description: 'Click Send' },
      { action: 'wait', condition: 'element', selector: '.storyb00k__message', timeout: 30000, description: 'Wait for response' },
      { action: 'assert', type: 'visible', selector: '.storyb00k__message', description: 'Verify message appeared' },
      { action: 'screenshot', name: 'agent-001-direct-request', description: 'Take screenshot' },
    ],
  },
  {
    name: 'Agent-002: Code Editor to Agent handoff',
    description: 'Verify Send to Agent button works',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'click', selector: '.view-tab:nth-child(2)', description: 'Click Code Editor tab' },
      { action: 'wait', condition: 'element', selector: 'textarea', description: 'Wait for editor' },
      { action: 'type', selector: 'textarea', text: 'x -> y -> z', description: 'Enter D2 source' },
      { action: 'click', selector: 'button:has-text("Render")', description: 'Render diagram' },
      { action: 'wait', condition: 'element', selector: 'img', timeout: 10000, description: 'Wait for render' },
      { action: 'click', selector: 'button:has-text("Send to Agent")', description: 'Click Send to Agent' },
      { action: 'wait', condition: 'element', selector: '[data-testid="starting-image"]', timeout: 5000, description: 'Wait for handoff preview' },
      { action: 'assert', type: 'visible', selector: '[data-testid="starting-image"]', description: 'Verify preview visible' },
      { action: 'screenshot', name: 'agent-002-handoff-preview', description: 'Screenshot with preview' },
      { action: 'type', selector: 'textarea', text: 'Review this diagram', description: 'Enter review request' },
      { action: 'click', selector: 'button:has-text("Send")', description: 'Click Send' },
      { action: 'wait', condition: 'element', selector: '.storyb00k__message', timeout: 30000, description: 'Wait for agent response' },
      { action: 'screenshot', name: 'agent-002-handoff-response', description: 'Screenshot with response' },
    ],
  },
  {
    name: 'Agent-003: Agent error handling',
    description: 'Verify agent handles invalid requests gracefully',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'click', selector: '.view-tab:nth-child(3)', description: 'Click Agent tab' },
      { action: 'wait', condition: 'element', selector: 'textarea', description: 'Wait for input' },
      { action: 'type', selector: 'textarea', text: 'Create a diagram with format=svg', description: 'Enter invalid request' },
      { action: 'click', selector: 'button:has-text("Send")', description: 'Click Send' },
      { action: 'wait', condition: 'element', selector: '.storyb00k__message', timeout: 30000, description: 'Wait for response' },
      { action: 'assert', type: 'visible', selector: '.storyb00k__message', description: 'Verify response appeared' },
      { action: 'screenshot', name: 'agent-003-error-handling', description: 'Take screenshot' },
    ],
  },
];

const UI_TESTS = [
  {
    name: 'UI-001: Navigation between views',
    description: 'Verify all view tabs work correctly',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'wait', condition: 'element', selector: '.view-tab', description: 'Wait for tabs' },
      { action: 'click', selector: '.view-tab:nth-child(1)', description: 'Click Gallery' },
      { action: 'wait', condition: 'element', selector: '.gallery', timeout: 5000, description: 'Wait for gallery' },
      { action: 'screenshot', name: 'ui-001-gallery', description: 'Screenshot gallery' },
      { action: 'click', selector: '.view-tab:nth-child(2)', description: 'Click Code Editor' },
      { action: 'wait', condition: 'element', selector: 'textarea', timeout: 5000, description: 'Wait for editor' },
      { action: 'screenshot', name: 'ui-001-editor', description: 'Screenshot editor' },
      { action: 'click', selector: '.view-tab:nth-child(3)', description: 'Click Agent' },
      { action: 'wait', condition: 'element', selector: '.storyb00k', timeout: 5000, description: 'Wait for agent' },
      { action: 'screenshot', name: 'ui-001-agent', description: 'Screenshot agent' },
      { action: 'click', selector: '.view-tab:nth-child(4)', description: 'Click Setup' },
      { action: 'wait', condition: 'element', selector: '.setup', timeout: 5000, description: 'Wait for setup' },
      { action: 'screenshot', name: 'ui-001-setup', description: 'Screenshot setup' },
    ],
  },
  {
    name: 'UI-002: Version display',
    description: 'Verify version is displayed in sidebar',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'wait', condition: 'element', selector: '.version-tag', description: 'Wait for version tag' },
      { action: 'assert', type: 'text', selector: '.version-tag', expected: 'v0.0.8', description: 'Verify version text' },
      { action: 'screenshot', name: 'ui-002-version', description: 'Take screenshot' },
    ],
  },
  {
    name: 'UI-003: Catalog loading',
    description: 'Verify example catalog loads correctly',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'wait', condition: 'element', selector: '.catalog-nav', description: 'Wait for catalog' },
      { action: 'assert', type: 'visible', selector: '.catalog-link', description: 'Verify catalog items visible' },
      { action: 'screenshot', name: 'ui-003-catalog', description: 'Take screenshot' },
    ],
  },
  {
    name: 'UI-004: Setup panel configuration',
    description: 'Verify setup panel can save settings',
    steps: [
      { action: 'navigate', url: `${CONFIG.baseUrl}/playbook/`, description: 'Navigate to playbook' },
      { action: 'click', selector: '.view-tab:nth-child(4)', description: 'Click Setup tab' },
      { action: 'wait', condition: 'element', selector: '.setup', description: 'Wait for setup panel' },
      { action: 'type', selector: 'input[placeholder*="8787"]', text: CONFIG.baseUrl, description: 'Enter renderer URL' },
      { action: 'click', selector: 'button:has-text("Save Settings")', description: 'Click Save' },
      { action: 'wait', condition: 'element', selector: 'button:has-text("✓ Saved")', timeout: 5000, description: 'Wait for save confirmation' },
      { action: 'screenshot', name: 'ui-004-setup-save', description: 'Take screenshot' },
    ],
  },
];

// ============================================================================
// TEST RUNNER
// ============================================================================

async function runTestSuite(suiteName, tests) {
  console.log(`\n${'='.repeat(60)}`);
  console.log(`📦 Running ${suiteName} Test Suite`);
  console.log(`${'='.repeat(60)}`);
  
  for (const test of tests) {
    await runTest(test);
  }
}

async function main() {
  const suite = process.argv[2] || 'all';
  
  console.log('🚀 kr0ki Functional Test Runner');
  console.log(`   Base URL: ${CONFIG.baseUrl}`);
  console.log(`   Agent URL: ${CONFIG.agentUrl}`);
  console.log(`   Suite: ${suite}`);
  
  // Initialize Chrome connection
  console.log('\n🔌 Connecting to Chrome DevTools...');
  const chromeStatus = chromeCommand('status');
  if (!chromeStatus) {
    console.error('❌ Failed to connect to Chrome DevTools');
    console.error('   Make sure Chrome is running with --remote-debugging-port=9222');
    process.exit(1);
  }
  console.log('   ✓ Connected to Chrome');
  
  // Run test suites
  if (suite === 'all' || suite === 'core') {
    await runTestSuite('Core', CORE_TESTS);
  }
  
  if (suite === 'all' || suite === 'agent') {
    await runTestSuite('Agent', AGENT_TESTS);
  }
  
  if (suite === 'all' || suite === 'ui') {
    await runTestSuite('UI', UI_TESTS);
  }
  
  // Print summary
  console.log(`\n${'='.repeat(60)}`);
  console.log('📊 Test Summary');
  console.log(`${'='.repeat(60)}`);
  console.log(`✅ Passed: ${results.passed.length}`);
  console.log(`❌ Failed: ${results.failed.length}`);
  console.log(`⏭️  Skipped: ${results.skipped.length}`);
  
  if (results.failed.length > 0) {
    console.log('\n❌ Failed Tests:');
    results.failed.forEach(f => {
      console.log(`   - ${f.name}: ${f.error}`);
    });
  }
  
  console.log(`\n📸 Screenshots saved to: ${CONFIG.screenshotDir}`);
  
  // Exit with appropriate code
  process.exit(results.failed.length > 0 ? 1 : 0);
}

main().catch(error => {
  console.error('Fatal error:', error);
  process.exit(1);
});
