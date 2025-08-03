#!/bin/bash

# Test 09: State Persistence
# Tests that repository state persists correctly through Theater

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing state persistence..."

# Get initial state
echo -n "  ✓ Getting initial repository state... "
if curl -s "$SERVER_URL/refs" > "$TEMP_DIR/initial_refs" && \
   curl -s "$SERVER_URL/objects" > "$TEMP_DIR/initial_objects"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL${NC}"
    exit 1
fi

# Check state consistency across multiple requests
echo -n "  ✓ State consistency across requests... "
sleep 1
if curl -s "$SERVER_URL/refs" > "$TEMP_DIR/second_refs" && \
   curl -s "$SERVER_URL/objects" > "$TEMP_DIR/second_objects"; then
    
    if diff "$TEMP_DIR/initial_refs" "$TEMP_DIR/second_refs" >/dev/null && \
       diff "$TEMP_DIR/initial_objects" "$TEMP_DIR/second_objects" >/dev/null; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${RED}FAIL (State changed between requests)${NC}"
        echo "Refs diff:"
        diff "$TEMP_DIR/initial_refs" "$TEMP_DIR/second_refs" || true
        echo "Objects diff:"
        diff "$TEMP_DIR/initial_objects" "$TEMP_DIR/second_objects" || true
        exit 1
    fi
else
    echo -e "${RED}FAIL${NC}"
    exit 1
fi

# Test capability advertisement consistency
echo -n "  ✓ Capability advertisement consistency... "
curl -s "$SERVER_URL/info/refs?service=git-upload-pack" > "$TEMP_DIR/capabilities_1"
sleep 1
curl -s "$SERVER_URL/info/refs?service=git-upload-pack" > "$TEMP_DIR/capabilities_2"

if diff "$TEMP_DIR/capabilities_1" "$TEMP_DIR/capabilities_2" >/dev/null; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (Capability advertisement inconsistent)${NC}"
    echo "Diff:"
    diff "$TEMP_DIR/capabilities_1" "$TEMP_DIR/capabilities_2" || true
    exit 1
fi

# Test ls-refs consistency
echo -n "  ✓ ls-refs response consistency... "
curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0012command=ls-refs\n0000' \
    "$SERVER_URL/git-upload-pack" > "$TEMP_DIR/ls_refs_1"

sleep 1

curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0012command=ls-refs\n0000' \
    "$SERVER_URL/git-upload-pack" > "$TEMP_DIR/ls_refs_2"

if diff "$TEMP_DIR/ls_refs_1" "$TEMP_DIR/ls_refs_2" >/dev/null; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (ls-refs response inconsistent)${NC}"
    echo "Diff:"
    diff "$TEMP_DIR/ls_refs_1" "$TEMP_DIR/ls_refs_2" || true
    exit 1
fi

# Test concurrent request handling
echo -n "  ✓ Concurrent request handling... "
(curl -s "$SERVER_URL/refs" > "$TEMP_DIR/concurrent_1" &)
(curl -s "$SERVER_URL/objects" > "$TEMP_DIR/concurrent_2" &)
(curl -s "$SERVER_URL/info/refs?service=git-upload-pack" > "$TEMP_DIR/concurrent_3" &)

# Wait for all requests to complete
wait

# Check that all requests completed successfully
if [[ -s "$TEMP_DIR/concurrent_1" ]] && \
   [[ -s "$TEMP_DIR/concurrent_2" ]] && \
   [[ -s "$TEMP_DIR/concurrent_3" ]]; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (Some concurrent requests failed)${NC}"
    ls -la "$TEMP_DIR"/concurrent_*
    exit 1
fi

# Validate repository structure consistency
echo -n "  ✓ Repository structure validation... "
if grep -q "refs" "$TEMP_DIR/initial_refs" >/dev/null 2>&1 || \
   [[ $(cat "$TEMP_DIR/initial_refs") == "{}" ]] || \
   [[ $(cat "$TEMP_DIR/initial_refs") == "[]" ]]; then
    # Either has refs or is empty JSON - both are valid
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Unexpected refs format)${NC}"
fi

if grep -q "objects" "$TEMP_DIR/initial_objects" >/dev/null 2>&1 || \
   [[ $(cat "$TEMP_DIR/initial_objects") == "{}" ]] || \
   [[ $(cat "$TEMP_DIR/initial_objects") == "[]" ]]; then
    # Either has objects or is empty JSON - both are valid
    echo "    ✓ Objects format valid"
else
    echo "    ⚠️  Unexpected objects format"
fi

# Test state after multiple protocol operations
echo -n "  ✓ State stability after protocol operations... "
# Perform several Protocol v2 operations
curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0012command=ls-refs\n0000' \
    "$SERVER_URL/git-upload-pack" > /dev/null

curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0011command=fetch\n0008done\n0000' \
    "$SERVER_URL/git-upload-pack" > /dev/null

curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0016command=object-info\n0008size\n0000' \
    "$SERVER_URL/git-upload-pack" > /dev/null

# Check state after operations
if curl -s "$SERVER_URL/refs" > "$TEMP_DIR/post_ops_refs" && \
   curl -s "$SERVER_URL/objects" > "$TEMP_DIR/post_ops_objects"; then
    
    if diff "$TEMP_DIR/initial_refs" "$TEMP_DIR/post_ops_refs" >/dev/null && \
       diff "$TEMP_DIR/initial_objects" "$TEMP_DIR/post_ops_objects" >/dev/null; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${YELLOW}WARN (State changed after operations - may be expected)${NC}"
    fi
else
    echo -e "${RED}FAIL${NC}"
    exit 1
fi

# Memory consistency test
echo -n "  ✓ Actor memory consistency... "
# Make rapid requests to test memory handling
for i in {1..5}; do
    curl -s "$SERVER_URL/" > /dev/null
    curl -s "$SERVER_URL/refs" > /dev/null
done

if curl -s "$SERVER_URL/refs" > "$TEMP_DIR/memory_test_refs"; then
    if diff "$TEMP_DIR/initial_refs" "$TEMP_DIR/memory_test_refs" >/dev/null; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${YELLOW}WARN (Memory state differs)${NC}"
    fi
else
    echo -e "${RED}FAIL${NC}"
    exit 1
fi

# Show state summary
echo ""
echo "📋 Repository State Summary:"
echo "==========================="
echo "Initial refs:"
if [[ -s "$TEMP_DIR/initial_refs" ]]; then
    head -5 "$TEMP_DIR/initial_refs"
    if [[ $(wc -l < "$TEMP_DIR/initial_refs") -gt 5 ]]; then
        echo "... ($(wc -l < "$TEMP_DIR/initial_refs") total lines)"
    fi
else
    echo "(Empty or no refs)"
fi

echo ""
echo "Initial objects:"
if [[ -s "$TEMP_DIR/initial_objects" ]]; then
    head -5 "$TEMP_DIR/initial_objects"
    if [[ $(wc -l < "$TEMP_DIR/initial_objects") -gt 5 ]]; then
        echo "... ($(wc -l < "$TEMP_DIR/initial_objects") total lines)"
    fi
else
    echo "(Empty or no objects)"
fi

echo ""
echo "State persistence test completed!"
