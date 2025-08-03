#!/bin/bash

# Test 01: Health Check
# Verifies basic server functionality

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing basic server health..."

# Test server root endpoint
echo -n "  ✓ Server root endpoint... "
if response=$(curl -s -w "%{http_code}" -o "$TEMP_DIR/root_response" "$SERVER_URL/"); then
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

# Test debug endpoints
echo -n "  ✓ Debug refs endpoint... "
if response=$(curl -s -w "%{http_code}" -o "$TEMP_DIR/refs_response" "$SERVER_URL/refs"); then
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

echo -n "  ✓ Debug objects endpoint... "
if response=$(curl -s -w "%{http_code}" -o "$TEMP_DIR/objects_response" "$SERVER_URL/objects"); then
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

# Check response content for debug info
echo -n "  ✓ Server returns JSON debug info... "
if grep -q "{" "$TEMP_DIR/root_response" 2>/dev/null; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (No JSON found)${NC}"
fi

echo "Health check completed successfully!"
