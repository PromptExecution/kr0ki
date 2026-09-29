#!/bin/bash
# Functional Test Runner for kr0ki
# Tests core functionality via API calls and validates expected outcomes

set -e

# Configuration
BASE_URL="${BASE_URL:-http://192.168.1.137:8787}"
AGENT_URL="${AGENT_URL:-http://192.168.1.137:8789}"
SCREENSHOT_DIR="/tmp/kr0ki-test-screenshots"
TEST_RESULTS="/tmp/kr0ki-test-results.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test result tracking
PASSED=0
FAILED=0
SKIPPED=0

# Ensure screenshot directory exists
mkdir -p "$SCREENSHOT_DIR"

# Initialize test results
echo '{"tests":[]}' > "$TEST_RESULTS"

# Helper functions
log_test() {
    echo -e "\n${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}TEST: $1${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

log_pass() {
    echo -e "${GREEN}✓ PASS: $1${NC}"
    PASSED=$((PASSED + 1))
    jq --arg name "$1" --arg status "passed" '.tests += [{"name": $name, "status": $status}]' "$TEST_RESULTS" > tmp.json && mv tmp.json "$TEST_RESULTS"
}

log_fail() {
    echo -e "${RED}✗ FAIL: $1${NC}"
    FAILED=$((FAILED + 1))
    jq --arg name "$1" --arg status "failed" --arg error "$2" '.tests += [{"name": $name, "status": $status, "error": $error}]' "$TEST_RESULTS" > tmp.json && mv tmp.json "$TEST_RESULTS"
}

log_skip() {
    echo -e "${YELLOW}⊘ SKIP: $1${NC}"
    SKIPPED=$((SKIPPED + 1))
    jq --arg name "$1" --arg status "skipped" '.tests += [{"name": $name, "status": $status}]' "$TEST_RESULTS" > tmp.json && mv tmp.json "$TEST_RESULTS"
}

# ============================================================================
# CORE TESTS
# ============================================================================

test_core_001() {
    log_test "CORE-001: kr0ki Server Health Check"
    
    response=$(curl -s "$BASE_URL/health")
    
    if echo "$response" | jq -e '.status == "ok"' > /dev/null; then
        log_pass "Server status is ok"
    else
        log_fail "Server status check" "Status is not ok"
        return 1
    fi
    
    if echo "$response" | jq -e '.version == "0.0.4"' > /dev/null; then
        log_pass "Version is 0.0.4"
    else
        log_fail "Version check" "Version is not 0.0.4"
        return 1
    fi
    
    if echo "$response" | jq -e '.service == "kr0ki"' > /dev/null; then
        log_pass "Service name is kr0ki"
    else
        log_fail "Service name check" "Service name is not kr0ki"
        return 1
    fi
}

test_core_002() {
    log_test "CORE-002: Agent Server Health Check"
    
    response=$(curl -s "$AGENT_URL/health")
    
    if echo "$response" | jq -e '.status == "ok"' > /dev/null; then
        log_pass "Agent status is ok"
    else
        log_fail "Agent status check" "Status is not ok"
        return 1
    fi
    
    if echo "$response" | jq -e '.service == "kr0ki-storyb00k-agent"' > /dev/null; then
        log_pass "Service name is correct"
    else
        log_fail "Service name check" "Service name is incorrect"
        return 1
    fi
    
    if echo "$response" | jq -e '.max_model_tool_rounds == 15' > /dev/null; then
        log_pass "Loop protection is active (max_model_tool_rounds=15)"
    else
        log_fail "Loop protection check" "max_model_tool_rounds is not 15"
        return 1
    fi
}

test_core_003() {
    log_test "CORE-003: Direct Diagram Render (D2)"
    
    output_file="$SCREENSHOT_DIR/core-003-d2-render.svg"
    
    http_code=$(curl -s -w "%{http_code}" -o "$output_file" \
        -X POST "$BASE_URL/render/d2?output=svg" \
        -H "Content-Type: text/plain" \
        -d 'x -> y -> z')
    
    if [ "$http_code" = "200" ]; then
        log_pass "HTTP status is 200"
    else
        log_fail "HTTP status check" "Expected 200, got $http_code"
        return 1
    fi
    
    if [ -f "$output_file" ] && [ -s "$output_file" ]; then
        log_pass "SVG file created and not empty"
    else
        log_fail "SVG file check" "File not created or empty"
        return 1
    fi
    
    if grep -q "<svg" "$output_file"; then
        log_pass "File contains valid SVG"
    else
        log_fail "SVG validation" "File does not contain SVG tags"
        return 1
    fi
}

test_core_004() {
    log_test "CORE-004: Catalog Loading"
    
    response=$(curl -s "$BASE_URL/api/examples")
    
    count=$(echo "$response" | jq 'length')
    
    if [ "$count" -gt 0 ]; then
        log_pass "Catalog contains $count examples"
    else
        log_fail "Catalog check" "Catalog is empty"
        return 1
    fi
    
    if echo "$response" | jq -e '.[0].format' > /dev/null; then
        log_pass "Examples have format field"
    else
        log_fail "Example structure" "Missing format field"
        return 1
    fi
}

# ============================================================================
# AGENT TESTS
# ============================================================================

test_agent_001() {
    log_test "AGENT-001: Direct Agent Request"
    
    response=$(curl -s -X POST "$AGENT_URL/run" \
        -H "Content-Type: application/json" \
        -d '{
            "threadId": "test-agent-001",
            "runId": "run-001",
            "messages": [
                {"role": "user", "content": "Create a simple flowchart with 3 boxes: Start, Process, End. Use D2 format."}
            ]
        }')
    
    if echo "$response" | grep -q "TOOL_CALL_START"; then
        log_pass "Agent made tool calls"
    else
        log_fail "Tool calls" "No tool calls detected"
        return 1
    fi
    
    if echo "$response" | grep -q "render_diagram"; then
        log_pass "Agent called render_diagram"
    else
        log_fail "render_diagram call" "Agent did not call render_diagram"
        return 1
    fi
    
    if echo "$response" | grep -q "RUN_FINISHED"; then
        log_pass "Run completed successfully"
    else
        log_fail "Run completion" "Run did not finish"
        return 1
    fi
}

test_agent_002() {
    log_test "AGENT-002: Editor Handoff Flow"
    
    # Step 1: Set diagram context
    context_response=$(curl -s -X POST "$AGENT_URL/projects/set-diagram-context" \
        -H "Content-Type: application/json" \
        -d '{
            "threadId": "test-agent-002",
            "source": "x -> y -> z",
            "format": "d2",
            "detectedType": "d2",
            "output": "svg",
            "imageData": null,
            "title": "Test Diagram"
        }')
    
    if echo "$context_response" | jq -e '.status == "ok"' > /dev/null; then
        log_pass "Context set successfully"
    else
        log_fail "Context setting" "Failed to set context"
        return 1
    fi
    
    # Step 2: Send message
    response=$(curl -s -X POST "$AGENT_URL/run" \
        -H "Content-Type: application/json" \
        -d '{
            "threadId": "test-agent-002",
            "runId": "run-002",
            "messages": [
                {"role": "user", "content": "Review this diagram and suggest improvements."}
            ]
        }')
    
    if echo "$response" | grep -q "TEXT_MESSAGE_CONTENT"; then
        log_pass "Agent provided response"
    else
        log_fail "Agent response" "No text message content"
        return 1
    fi
    
    if echo "$response" | grep -q "RUN_FINISHED"; then
        log_pass "Run completed successfully"
    else
        log_fail "Run completion" "Run did not finish"
        return 1
    fi
}

test_agent_003() {
    log_test "AGENT-003: Agent Error Handling (Invalid Format)"
    
    response=$(curl -s -X POST "$AGENT_URL/run" \
        -H "Content-Type: application/json" \
        -d '{
            "threadId": "test-agent-003",
            "runId": "run-003",
            "messages": [
                {"role": "user", "content": "Create a diagram with format=svg"}
            ]
        }')
    
    # Should complete without infinite loop
    if echo "$response" | grep -q "RUN_FINISHED\|RUN_ERROR"; then
        log_pass "Run completed (no infinite loop)"
    else
        log_fail "Loop protection" "Run did not complete"
        return 1
    fi
    
    # Should mention format error
    if echo "$response" | grep -qi "svg\|format\|output"; then
        log_pass "Error message mentions format issue"
    else
        log_fail "Error message" "No format-related error message"
        return 1
    fi
}

# ============================================================================
# UI TESTS (API-based validation)
# ============================================================================

test_ui_001() {
    log_test "UI-001: Playbook HTML Loads"
    
    http_code=$(curl -s -w "%{http_code}" -o /dev/null "$BASE_URL/playbook/")
    
    if [ "$http_code" = "200" ]; then
        log_pass "Playbook loads with HTTP 200"
    else
        log_fail "Playbook load" "HTTP $http_code"
        return 1
    fi
}

test_ui_002() {
    log_test "UI-002: Version in Built Files"
    
    if grep -q "0.0.4" playbook/dist/assets/*.js 2>/dev/null; then
        log_pass "Version 0.0.4 found in built files"
    else
        log_fail "Version check" "Version 0.0.4 not found in built files"
        return 1
    fi
}

test_ui_003() {
    log_test "UI-003: Setup Panel Exists"
    
    if grep -q "Setup" playbook/src/App.vue; then
        log_pass "Setup component exists in App.vue"
    else
        log_fail "Setup component" "Setup component not found"
        return 1
    fi
}

test_ui_004() {
    log_test "UI-004: Send to Agent Button Exists"
    
    if grep -q "Send to Agent" playbook/src/components/RendererPanel.vue; then
        log_pass "Send to Agent button exists"
    else
        log_fail "Send to Agent button" "Button not found"
        return 1
    fi
}

test_ui_005() {
    log_test "UI-005: Auto Render Checkbox Exists"
    
    if grep -q "Auto Render" playbook/src/components/RendererPanel.vue; then
        log_pass "Auto Render checkbox exists"
    else
        log_fail "Auto Render checkbox" "Checkbox not found"
        return 1
    fi
}

# ============================================================================
# INTEGRATION TESTS
# ============================================================================

test_int_001() {
    log_test "INT-001: End-to-End Flow"
    
    # Render a diagram
    render_response=$(curl -s -X POST "$BASE_URL/render/d2?output=svg" \
        -H "Content-Type: text/plain" \
        -d 'start -> process -> end')
    
    if echo "$render_response" | grep -q "<svg"; then
        log_pass "Diagram rendered successfully"
    else
        log_fail "Diagram render" "Render failed"
        return 1
    fi
    
    # Send to agent
    agent_response=$(curl -s -X POST "$AGENT_URL/run" \
        -H "Content-Type: application/json" \
        -d '{
            "threadId": "test-int-001",
            "runId": "run-int-001",
            "messages": [
                {"role": "user", "content": "I have a D2 diagram: start -> process -> end. Review it."}
            ]
        }')
    
    if echo "$agent_response" | grep -q "TEXT_MESSAGE_CONTENT"; then
        log_pass "Agent responded to diagram"
    else
        log_fail "Agent response" "No response from agent"
        return 1
    fi
}

# ============================================================================
# MAIN
# ============================================================================

main() {
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "kr0ki Functional Test Suite"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Base URL: $BASE_URL"
    echo "Agent URL: $AGENT_URL"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Check prerequisites
    if ! curl -s "$BASE_URL/health" > /dev/null 2>&1; then
        echo -e "${RED}ERROR: kr0ki server not reachable at $BASE_URL${NC}"
        exit 1
    fi
    
    if ! curl -s "$AGENT_URL/health" > /dev/null 2>&1; then
        echo -e "${RED}ERROR: Agent server not reachable at $AGENT_URL${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ All services reachable${NC}"
    
    # Run test suites
    echo -e "\n${YELLOW}Running Core Tests...${NC}"
    test_core_001 || true
    test_core_002 || true
    test_core_003 || true
    test_core_004 || true
    
    echo -e "\n${YELLOW}Running Agent Tests...${NC}"
    test_agent_001 || true
    test_agent_002 || true
    test_agent_003 || true
    
    echo -e "\n${YELLOW}Running UI Tests...${NC}"
    test_ui_001 || true
    test_ui_002 || true
    test_ui_003 || true
    test_ui_004 || true
    test_ui_005 || true
    
    echo -e "\n${YELLOW}Running Integration Tests...${NC}"
    test_int_001 || true
    
    # Print summary
    echo -e "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Test Summary"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "${GREEN}Passed: $PASSED${NC}"
    echo -e "${RED}Failed: $FAILED${NC}"
    echo -e "${YELLOW}Skipped: $SKIPPED${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    if [ $FAILED -gt 0 ]; then
        echo -e "\n${RED}Failed Tests:${NC}"
        jq -r '.tests[] | select(.status == "failed") | "  - \(.name): \(.error)"' "$TEST_RESULTS"
    fi
    
    echo -e "\nScreenshots saved to: $SCREENSHOT_DIR"
    echo "Test results saved to: $TEST_RESULTS"
    
    # Exit with appropriate code
    if [ $FAILED -gt 0 ]; then
        exit 1
    else
        exit 0
    fi
}

main "$@"
