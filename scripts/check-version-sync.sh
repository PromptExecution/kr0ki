#!/usr/bin/env bash
# check-version-sync.sh - Ensure version consistency across the project
#
# Verifies that the version in Cargo.toml is properly reflected in
# the playbook UI and other relevant locations.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Extract version from Cargo.toml
CARGO_VERSION=$(grep "^version" Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

echo "Checking version sync..."
echo "Cargo.toml version: $CARGO_VERSION"

# Check if playbook Vite config reads from Cargo.toml
if grep -q "readFileSync.*Cargo.toml" playbook/vite.config.js 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Playbook Vite config reads version from Cargo.toml"
else
    echo -e "${YELLOW}⚠${NC} Playbook Vite config may not be reading version from Cargo.toml"
fi

# Check if App.vue uses __APP_VERSION__
if grep -q "__APP_VERSION__" playbook/src/App.vue 2>/dev/null; then
    echo -e "${GREEN}✓${NC} App.vue uses __APP_VERSION__ constant"
else
    echo -e "${YELLOW}⚠${NC} App.vue may not be displaying version"
fi

# Check if version is displayed in UI
if grep -q "appVersion" playbook/src/App.vue 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Version is displayed in the UI"
else
    echo -e "${YELLOW}⚠${NC} Version may not be displayed in the UI"
fi

echo ""
echo "Version sync check complete"
