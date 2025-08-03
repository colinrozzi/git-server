#!/bin/bash

# Test 02: Protocol v2 Capability Advertisement
# Verifies that the server properly advertises Git Protocol v2 capabilities

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing Protocol v2 capability advertisement..."

# Test smart HTTP info/refs endpoint
echo -n "  ✓ Smart HTTP info/refs responds... "
if response=$(curl -s -w "%{http_code}" -o "$TEMP_DIR/capabilities" "$SERVER_URL/info/refs?service=git-upload-pack"); then
    status_code="${response: -3}"
    if [[ "$status_code" == "200" ]]; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Check for Protocol v2 version announcement
echo -n "  ✓ Advertises Protocol v2... "
if grep -q "version 2" "$TEMP_DIR/capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (Protocol v2 not found)${NC}"
    echo "Response content:"
    cat "$TEMP_DIR/capabilities"
    exit 1
fi

# Check for agent string
echo -n "  ✓ Includes agent string... "
if grep -q "agent=git-server" "$TEMP_DIR/capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Agent string not found)${NC}"
fi

# Check for object format
echo -n "  ✓ Declares object format... "
if grep -q "object-format=sha1" "$TEMP_DIR/capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Object format not declared)${NC}"
fi

# Check for ls-refs capability
echo -n "  ✓ Advertises ls-refs capability... "
if grep -q "ls-refs" "$TEMP_DIR/capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (ls-refs capability missing)${NC}"
    exit 1
fi

# Check for fetch capability
echo -n "  ✓ Advertises fetch capability... "
if grep -q "fetch" "$TEMP_DIR/capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (fetch capability missing)${NC}"
    exit 1
fi

# Check for object-info capability
echo -n "  ✓ Advertises object-info capability... "
if grep -q "object-info" "$TEMP_DIR/capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (object-info capability missing)${NC}"
fi

# Validate packet-line format
echo -n "  ✓ Uses proper packet-line format... "
if head -c 4 "$TEMP_DIR/capabilities" | grep -E '^[0-9a-f]{4}$' >/dev/null; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL (Invalid packet-line format)${NC}"
    echo "First 4 bytes:"
    head -c 4 "$TEMP_DIR/capabilities" | hexdump -C
    exit 1
fi

# Show capability summary
echo ""
echo "📋 Capability Summary:"
echo "======================"
cat "$TEMP_DIR/capabilities" | head -20

echo ""
echo "Protocol v2 capability advertisement test completed!"
