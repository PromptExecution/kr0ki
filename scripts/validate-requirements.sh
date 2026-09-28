#!/usr/bin/env bash
# validate-requirements.sh - Validate requirement documents
#
# Checks that requirement files follow the expected format and contain
# all required sections.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

ERRORS=0

for file in "$@"; do
    if [[ ! -f "$file" ]]; then
        continue
    fi
    
    # Skip README and TEMPLATE
    if [[ "$file" == *"README.md" ]] || [[ "$file" == *"TEMPLATE.md" ]]; then
        continue
    fi
    
    echo "Validating: $file"
    
    # Check for required sections
    if ! grep -q "^# " "$file"; then
        echo -e "${RED}✗${NC} Missing main title"
        ERRORS=$((ERRORS + 1))
    fi
    
    if ! grep -q "^## Overview" "$file"; then
        echo -e "${YELLOW}⚠${NC} Missing Overview section"
    fi
    
    # Check for requirement IDs
    if ! grep -qE "^### (FR|NFR|IR|CR)-[0-9]+" "$file"; then
        echo -e "${YELLOW}⚠${NC} No properly formatted requirements found (expected: ### FR-001, NFR-001, etc.)"
    fi
    
    # Check for priority fields
    if grep -q "^### " "$file" && ! grep -q "^\*\*Priority:\*\*" "$file"; then
        echo -e "${YELLOW}⚠${NC} Some requirements missing Priority field"
    fi
    
    # Check for acceptance criteria
    if grep -q "^### " "$file" && ! grep -q "^\*\*Acceptance Criteria:\*\*" "$file"; then
        echo -e "${YELLOW}⚠${NC} Some requirements missing Acceptance Criteria"
    fi
    
    echo -e "${GREEN}✓${NC} Validation complete"
done

if [ $ERRORS -gt 0 ]; then
    echo ""
    echo -e "${RED}Found $ERRORS error(s)${NC}"
    exit 1
fi

exit 0
