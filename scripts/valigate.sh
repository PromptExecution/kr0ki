#!/usr/bin/env bash
# valigate.sh - validate + gate + date
# A small circuit of conditions and contextually aware governance documents
#
# This script performs pre-commit validation to ensure code quality,
# requirement compliance, and proper versioning before allowing commits.

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    VALIGATE CIRCUIT                           ║${NC}"
echo -e "${BLUE}║              validate + gate + date                           ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Track validation status
ERRORS=0
WARNINGS=0

# Function to log success
pass() {
    echo -e "${GREEN}✓${NC} $1"
}

# Function to log failure
fail() {
    echo -e "${RED}✗${NC} $1"
    ERRORS=$((ERRORS + 1))
}

# Function to log warning
warn() {
    echo -e "${YELLOW}⚠${NC} $1"
    WARNINGS=$((WARNINGS + 1))
}

# Function to log info
info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

echo -e "${BLUE}[1/6] Checking Rust formatting...${NC}"
if cargo fmt --all -- --check > /dev/null 2>&1; then
    pass "Rust code is properly formatted"
else
    fail "Rust code needs formatting (run: cargo fmt --all)"
fi

echo ""
echo -e "${BLUE}[2/6] Running Rust linter...${NC}"
if cargo clippy --workspace --all-targets -- -D warnings > /dev/null 2>&1; then
    pass "No clippy warnings"
else
    fail "Clippy found warnings (run: cargo clippy --workspace --all-targets)"
fi

echo ""
echo -e "${BLUE}[3/6] Running Rust tests...${NC}"
if cargo test --workspace --quiet > /dev/null 2>&1; then
    pass "All tests pass"
else
    fail "Tests failed (run: cargo test --workspace)"
fi

echo ""
echo -e "${BLUE}[4/6] Validating requirements...${NC}"
if [ -d "docs/requirements" ]; then
    # Check for required files
    if [ -f "docs/requirements/README.md" ]; then
        pass "Requirements README exists"
    else
        fail "Missing docs/requirements/README.md"
    fi
    
    if [ -f "docs/requirements/TEMPLATE.md" ]; then
        pass "Requirements template exists"
    else
        fail "Missing docs/requirements/TEMPLATE.md"
    fi
    
    if [ -f "docs/requirements/system-normal.md" ]; then
        pass "System-normal requirements exist"
        
        # Validate requirement format
        if grep -q "^### FR-" docs/requirements/system-normal.md || \
           grep -q "^### NFR-" docs/requirements/system-normal.md || \
           grep -q "^### IR-" docs/requirements/system-normal.md || \
           grep -q "^### CR-" docs/requirements/system-normal.md; then
            pass "Requirements follow naming convention"
        else
            warn "No properly formatted requirements found"
        fi
    else
        warn "No system-normal requirements file yet"
    fi
else
    warn "No requirements directory found"
fi

echo ""
echo -e "${BLUE}[5/6] Checking version sync...${NC}"
# Extract version from Cargo.toml
CARGO_VERSION=$(grep "^version" Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
info "Cargo.toml version: $CARGO_VERSION"

# Check if playbook has the version
if grep -q "__APP_VERSION__" playbook/vite.config.js 2>/dev/null; then
    pass "Playbook Vite config reads version from Cargo.toml"
else
    warn "Playbook may not be synced with Cargo version"
fi

echo ""
echo -e "${BLUE}[6/6] Checking for sensitive data...${NC}"
# Check for common sensitive patterns
if git diff --cached --name-only | xargs grep -l "password\|secret\|api_key\|token" 2>/dev/null | grep -v ".env.example" | grep -v "README" > /dev/null; then
    warn "Potential sensitive data in staged files (review carefully)"
else
    pass "No obvious sensitive data detected"
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""

# Final verdict
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                    ✓ VALIGATE PASSED                          ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    if [ $WARNINGS -gt 0 ]; then
        echo -e "${YELLOW}  $WARNINGS warning(s) - review recommended${NC}"
    fi
    echo -e "${GREEN}  Commit is gated and dated: $(date -u +'%Y-%m-%dT%H:%M:%SZ')${NC}"
    echo ""
    exit 0
else
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                    ✗ VALIGATE FAILED                          ║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${RED}  $ERRORS error(s) found - commit blocked${NC}"
    if [ $WARNINGS -gt 0 ]; then
        echo -e "${YELLOW}  $WARNINGS warning(s) - review recommended${NC}"
    fi
    echo ""
    exit 1
fi
