#!/bin/bash
# Integration test for Milestone 10: Guardrails & CI Integration

set -e  # Exit on error

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "======================================================================"
echo "Milestone 10 Integration Tests"
echo "======================================================================"

# Test 1: Complexity Check
echo ""
echo "Test 1: Complexity Check"
echo "----------------------------------------------------------------------"
python3 scripts/validate.py check-complexity
echo "✓ Complexity check completed"

# Test 2: Show Effective Rules - CLAUDE.md
echo ""
echo "Test 2: Show Effective Rules - CLAUDE.md"
echo "----------------------------------------------------------------------"
OUTPUT=$(python3 scripts/validate.py validate-claude-md --show-effective-rules)
if echo "$OUTPUT" | grep -q "Effective Validation Rules for CLAUDE.md"; then
    echo "✓ CLAUDE.md effective rules displayed correctly"
else
    echo "✗ CLAUDE.md effective rules not displayed"
    exit 1
fi

# Test 3: Show Effective Rules - Hub Documents
echo ""
echo "Test 3: Show Effective Rules - Hub Documents"
echo "----------------------------------------------------------------------"
OUTPUT=$(python3 scripts/validate.py validate-hub --show-effective-rules)
if echo "$OUTPUT" | grep -q "Effective Validation Rules for Hub Documents"; then
    echo "✓ Hub effective rules displayed correctly"
else
    echo "✗ Hub effective rules not displayed"
    exit 1
fi

# Test 4: Show Effective Rules - Spoke Documents
echo ""
echo "Test 4: Show Effective Rules - Spoke Documents"
echo "----------------------------------------------------------------------"
OUTPUT=$(python3 scripts/validate.py validate-spoke --show-effective-rules)
if echo "$OUTPUT" | grep -q "Effective Validation Rules for Spoke Documents"; then
    echo "✓ Spoke effective rules displayed correctly"
else
    echo "✗ Spoke effective rules not displayed"
    exit 1
fi

# Test 5: Verify CI Workflow File Exists
echo ""
echo "Test 5: Verify CI Workflow File"
echo "----------------------------------------------------------------------"
if [ -f "../.github/workflows/validate-docs.yml" ]; then
    echo "✓ CI workflow file exists"
else
    echo "✗ CI workflow file not found"
    exit 1
fi

# Test 6: Verify Requirements File Updated
echo ""
echo "Test 6: Verify Requirements File"
echo "----------------------------------------------------------------------"
if grep -q "PyYAML" scripts/requirements.txt && grep -q "jsonschema" scripts/requirements.txt; then
    echo "✓ Requirements file contains all dependencies"
else
    echo "✗ Requirements file missing dependencies"
    exit 1
fi

echo ""
echo "======================================================================"
echo "All Milestone 10 Tests Passed!"
echo "======================================================================"
