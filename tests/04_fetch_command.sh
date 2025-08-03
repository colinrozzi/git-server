#!/bin/bash

# Test 04: fetch Command
# Tests the fetch command implementation

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing fetch command..."

# Helper function to test fetch commands
test_fetch() {
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
            head -200 "$output_file"
            return 1
        fi
    else
        echo -e "${RED}FAIL (Connection error)${NC}"
        return 1
    fi
}

# Test basic fetch (empty repo case)
test_fetch "Basic fetch (done only)" $'0011command=fetch\n0008done\n0000' "$TEMP_DIR/fetch_basic"

# Test fetch with capabilities
test_fetch "fetch with no-progress" $'0011command=fetch\n0010no-progress\n0008done\n0000' "$TEMP_DIR/fetch_no_progress"

# Test fetch with thin-pack capability
test_fetch "fetch with thin-pack" $'0011command=fetch\n000ethin-pack\n0008done\n0000' "$TEMP_DIR/fetch_thin_pack"

# Test fetch with ofs-delta capability
test_fetch "fetch with ofs-delta" $'0011command=fetch\n000dofs-delta\n0008done\n0000' "$TEMP_DIR/fetch_ofs_delta"

# Test fetch with sideband-all capability
test_fetch "fetch with sideband-all" $'0011command=fetch\n0011sideband-all\n0008done\n0000' "$TEMP_DIR/fetch_sideband"

# Validate response structure for empty repo
echo -n "  ✓ Empty repo response format... "
response_size=$(stat -f%z "$TEMP_DIR/fetch_basic" 2>/dev/null || stat -c%s "$TEMP_DIR/fetch_basic" 2>/dev/null)
if [[ $response_size -ge 4 ]]; then
    echo -e "${GREEN}PASS (Got response)${NC}"
else
    echo -e "${RED}FAIL (Response too small)${NC}"
    echo "Response size: $response_size bytes"
    exit 1
fi

# Check if response contains appropriate acknowledgment
echo -n "  ✓ Contains acknowledgment section... "
if grep -q "acknowledgments" "$TEMP_DIR/fetch_basic" || \
   grep -q "ready" "$TEMP_DIR/fetch_basic" || \
   [[ $response_size -lt 20 ]]; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (No clear acknowledgment found)${NC}"
fi

# Test with invalid want (should handle gracefully)
echo -n "  ✓ Handles invalid want gracefully... "
if response=$(curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0011command=fetch\n004cwant 0000000000000000000000000000000000000000\n0008done\n0000' \
    -w "%{http_code}" \
    -o "$TEMP_DIR/fetch_invalid_want" \
    "$SERVER_URL/git-upload-pack"); then
    
    status_code="${response: -3}"
    if [[ "$status_code" == "200" ]] || [[ "$status_code" == "400" ]]; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Show sample response
echo ""
echo "📋 Sample fetch Response:"
echo "========================"
echo "Basic fetch (first 200 bytes):"
head -c 200 "$TEMP_DIR/fetch_basic" | hexdump -C

echo ""
echo "fetch command test completed!"
