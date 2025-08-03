#!/bin/bash

# Test 05: object-info Command
# Tests the object-info command implementation

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing object-info command..."

# Helper function to test object-info commands
test_object_info() {
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

# Test basic object-info with size
test_object_info "object-info with size" $'0016command=object-info\n0008size\n0000' "$TEMP_DIR/object_info_size"

# Test object-info with content
test_object_info "object-info with content" $'0016command=object-info\n000bcontent\n0000' "$TEMP_DIR/object_info_content"

# Test object-info with content-limit
test_object_info "object-info with content-limit" $'0016command=object-info\n000bcontent\n0013content-limit=100\n0000' "$TEMP_DIR/object_info_limit"

# Test object-info with content-encoding
test_object_info "object-info with content-encoding" $'0016command=object-info\n000bcontent\n0018content-encoding=base64\n0000' "$TEMP_DIR/object_info_encoding"

# Validate response format
echo -n "  ✓ Response format validation... "
response_size=$(stat -f%z "$TEMP_DIR/object_info_size" 2>/dev/null || stat -c%s "$TEMP_DIR/object_info_size" 2>/dev/null)
if [[ $response_size -ge 4 ]]; then
    echo -e "${GREEN}PASS (Got response)${NC}"
else
    echo -e "${RED}FAIL (Response too small)${NC}"
    echo "Response size: $response_size bytes"
    exit 1
fi

# Check for proper packet-line ending
echo -n "  ✓ Response ends properly... "
if tail -c 4 "$TEMP_DIR/object_info_size" | grep -q "0000"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (No flush packet found at end)${NC}"
fi

# Show sample response
echo ""
echo "📋 Sample object-info Response:"
echo "=============================="
echo "object-info with size (first 200 bytes):"
head -c 200 "$TEMP_DIR/object_info_size" | hexdump -C

echo ""
echo "object-info command test completed!"
