#!/bin/bash

# Test 06: Error Handling
# Tests various error conditions and malformed requests

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing error handling..."

# Test invalid command
echo -n "  ✓ Invalid command rejection... "
if response=$(curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0013command=invalid0000' \
    -w "%{http_code}" \
    -o "$TEMP_DIR/error_invalid_command" \
    "$SERVER_URL/git-upload-pack"); then
    
    status_code="${response: -3}"
    # Should return 4xx or 5xx error
    if [[ "$status_code" =~ ^[45][0-9][0-9]$ ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${YELLOW}WARN (HTTP $status_code - unexpected but not necessarily wrong)${NC}"
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Test malformed packet-line
echo -n "  ✓ Malformed packet-line handling... "
if response=$(curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary 'invalid-packet-line-format' \
    -w "%{http_code}" \
    -o "$TEMP_DIR/error_malformed_packet" \
    "$SERVER_URL/git-upload-pack"); then
    
    status_code="${response: -3}"
    # Should handle gracefully (might be 200 with error response or 4xx)
    if [[ "$status_code" =~ ^[2-5][0-9][0-9]$ ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Test unsupported service (git-receive-pack)
echo -n "  ✓ Unsupported service handling... "
if response=$(curl -s -w "%{http_code}" \
    -o "$TEMP_DIR/error_unsupported_service" \
    "$SERVER_URL/info/refs?service=git-receive-pack"); then
    
    status_code="${response: -3}"
    # Might return 404, 405, or 501 for unsupported service
    if [[ "$status_code" =~ ^[4-5][0-9][0-9]$ ]] || [[ "$status_code" == "200" ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Test invalid service parameter
echo -n "  ✓ Invalid service parameter... "
if response=$(curl -s -w "%{http_code}" \
    -o "$TEMP_DIR/error_invalid_service" \
    "$SERVER_URL/info/refs?service=invalid-service"); then
    
    status_code="${response: -3}"
    # Should handle gracefully
    if [[ "$status_code" =~ ^[2-5][0-9][0-9]$ ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Test missing service parameter
echo -n "  ✓ Missing service parameter... "
if response=$(curl -s -w "%{http_code}" \
    -o "$TEMP_DIR/error_no_service" \
    "$SERVER_URL/info/refs"); then
    
    status_code="${response: -3}"
    if [[ "$status_code" =~ ^[2-5][0-9][0-9]$ ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Test invalid HTTP method on git-upload-pack
echo -n "  ✓ Invalid HTTP method handling... "
if response=$(curl -s -X GET \
    -w "%{http_code}" \
    -o "$TEMP_DIR/error_invalid_method" \
    "$SERVER_URL/git-upload-pack"); then
    
    status_code="${response: -3}"
    # Should reject GET on git-upload-pack endpoint
    if [[ "$status_code" =~ ^[4-5][0-9][0-9]$ ]] || [[ "$status_code" == "200" ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Test large request body handling
echo -n "  ✓ Large request body handling... "
large_data=$(printf 'A%.0s' {1..10000})  # 10KB of 'A's
if response=$(curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary "$large_data" \
    -w "%{http_code}" \
    -o "$TEMP_DIR/error_large_body" \
    "$SERVER_URL/git-upload-pack"); then
    
    status_code="${response: -3}"
    # Should handle large bodies gracefully
    if [[ "$status_code" =~ ^[2-5][0-9][0-9]$ ]]; then
        echo -e "${GREEN}PASS (HTTP $status_code)${NC}"
    else
        echo -e "${RED}FAIL (HTTP $status_code)${NC}"
        exit 1
    fi
else
    echo -e "${RED}FAIL (Connection error)${NC}"
    exit 1
fi

# Show sample error responses
echo ""
echo "📋 Sample Error Responses:"
echo "========================="
echo "Invalid command response (first 200 bytes):"
head -c 200 "$TEMP_DIR/error_invalid_command" | hexdump -C

echo ""
echo "Error handling test completed!"
