#!/bin/bash

# Test 03: ls-refs Command
# Tests the ls-refs command implementation

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing ls-refs command..."

# Helper function to test ls-refs commands
test_ls_refs() {
    local test_name="$1"
    local request_data="$2"
    local output_file="$3"
    
    echo -n "  ✓ $test_name... "
    
    if response=$(curl -s -X POST \
        -H "Content-Type: application/x-git-upload-pack-request" \
        --data-binary "$request_data" \
        -w "%{http_code}" \
        -o "$output_file" \
        "$SERVER_URL/git-upload-pack"); then
        
        status_code="${response: -3}"
        if [[ "$status_code" == "200" ]]; then
            echo -e "${GREEN}PASS${NC}"
            return 0
        else
            echo -e "${RED}FAIL (HTTP $status_code)${NC}"
            echo "Response:"
            cat "$output_file"
            return 1
        fi
    else
        echo -e "${RED}FAIL (Connection error)${NC}"
        return 1
    fi
}

# Test basic ls-refs command
test_ls_refs "Basic ls-refs" $'0013command=ls-refs0000' "$TEMP_DIR/ls_refs_basic"

# Test ls-refs with symrefs option
test_ls_refs "ls-refs with symrefs" $'0013command=ls-refs000bsymrefs0000' "$TEMP_DIR/ls_refs_symrefs"

# Test ls-refs with ref-prefix filtering
test_ls_refs "ls-refs with ref-prefix" $'0013command=ls-refs001aref-prefix refs/heads/0000' "$TEMP_DIR/ls_refs_prefix"

# Test ls-refs with peel option
test_ls_refs "ls-refs with peel" $'0013command=ls-refs0008peel0000' "$TEMP_DIR/ls_refs_peel"

# Test ls-refs with unborn option
test_ls_refs "ls-refs with unborn" $'0013command=ls-refs000aunborn0000' "$TEMP_DIR/ls_refs_unborn"

# Validate response format
echo -n "  ✓ Response ends with flush packet... "
if tail -c 4 "$TEMP_DIR/ls_refs_basic" | grep -q "0000"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (No flush packet found)${NC}"
    echo "Last 10 bytes:"
    tail -c 10 "$TEMP_DIR/ls_refs_basic" | hexdump -C
    exit 1
fi

# For empty repository, check that we get an appropriate response
echo -n "  ✓ Empty repository handling... "
response_size=$(stat -f%z "$TEMP_DIR/ls_refs_basic" 2>/dev/null || stat -c%s "$TEMP_DIR/ls_refs_basic" 2>/dev/null)
if [[ $response_size -ge 4 ]]; then
    echo -e "${GREEN}PASS (Got response)${NC}"
else
    echo -e "${RED}FAIL (Response too small)${NC}"
    exit 1
fi

# Show sample response
echo ""
echo "📋 Sample ls-refs Response:"
echo "=========================="
echo "Basic ls-refs (first 200 bytes):"
head -c 200 "$TEMP_DIR/ls_refs_basic" | hexdump -C

echo ""
echo "ls-refs command test completed!"
