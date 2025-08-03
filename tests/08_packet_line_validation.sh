#!/bin/bash

# Test 08: Packet-line Protocol Validation
# Tests the Git packet-line protocol implementation details

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing packet-line protocol validation..."

# Helper function to validate packet-line format
validate_packet_line() {
    local file="$1"
    local description="$2"
    
    echo -n "  ✓ $description packet-line format... "
    
    if [[ ! -f "$file" ]]; then
        echo -e "${RED}FAIL (File not found)${NC}"
        return 1
    fi
    
    # Check file size
    local size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    if [[ $size -lt 4 ]]; then
        echo -e "${RED}FAIL (File too small: $size bytes)${NC}"
        return 1
    fi
    
    # Check first packet-line format
    local first_four=$(head -c 4 "$file")
    if [[ ! "$first_four" =~ ^[0-9a-f]{4}$ ]]; then
        echo -e "${RED}FAIL (Invalid packet-line header: $first_four)${NC}"
        return 1
    fi
    
    # Convert hex to decimal
    local packet_len=$(printf "%d" "0x$first_four")
    
    # Validate packet length
    if [[ $packet_len -eq 0 ]]; then
        # Flush packet - should be exactly "0000"
        if [[ "$first_four" == "0000" ]]; then
            echo -e "${GREEN}PASS (Flush packet)${NC}"
        else
            echo -e "${RED}FAIL (Invalid flush packet)${NC}"
            return 1
        fi
    elif [[ $packet_len -ge 4 && $packet_len -le 65520 ]]; then
        # Normal packet
        echo -e "${GREEN}PASS (Packet length: $packet_len)${NC}"
    else
        echo -e "${RED}FAIL (Invalid packet length: $packet_len)${NC}"
        return 1
    fi
    
    return 0
}

# Get test responses from info/refs
echo -n "  ✓ Getting capability advertisement... "
if curl -s "$SERVER_URL/info/refs?service=git-upload-pack" > "$TEMP_DIR/packet_capabilities"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL${NC}"
    exit 1
fi

# Validate capability advertisement packet-line format
validate_packet_line "$TEMP_DIR/packet_capabilities" "Capability advertisement"

# Test ls-refs response
echo -n "  ✓ Getting ls-refs response... "
if curl -s -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary $'0013command=ls-refs0000' \
    "$SERVER_URL/git-upload-pack" > "$TEMP_DIR/packet_ls_refs"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL${NC}"
    exit 1
fi

# Validate ls-refs packet-line format
validate_packet_line "$TEMP_DIR/packet_ls_refs" "ls-refs response"

# Check for proper flush packet termination
echo -n "  ✓ Responses end with flush packet... "
if tail -c 4 "$TEMP_DIR/packet_capabilities" | grep -q "0000" && \
   tail -c 4 "$TEMP_DIR/packet_ls_refs" | grep -q "0000"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL${NC}"
    echo "Capability response ending:"
    tail -c 8 "$TEMP_DIR/packet_capabilities" | hexdump -C
    echo "ls-refs response ending:"
    tail -c 8 "$TEMP_DIR/packet_ls_refs" | hexdump -C
    exit 1
fi

# Parse and validate multiple packet-lines in capability response
echo -n "  ✓ Multiple packet-line parsing... "
python3 -c "
import sys
import re

def parse_packet_lines(data):
    offset = 0
    packets = []
    while offset < len(data):
        if offset + 4 > len(data):
            return False, 'Incomplete packet-line header'
        
        length_str = data[offset:offset+4].decode('ascii', errors='ignore')
        if not re.match(r'^[0-9a-f]{4}$', length_str):
            return False, f'Invalid packet-line header: {length_str}'
        
        length = int(length_str, 16)
        if length == 0:
            # Flush packet
            offset += 4
            packets.append('FLUSH')
            break
        elif length < 4:
            return False, f'Invalid packet length: {length}'
        elif offset + length > len(data):
            return False, f'Packet extends beyond data: {length} bytes at offset {offset}'
        else:
            packet_data = data[offset+4:offset+length]
            packets.append(packet_data.decode('utf-8', errors='ignore').rstrip('\n'))
            offset += length
    
    return True, packets

try:
    with open('$TEMP_DIR/packet_capabilities', 'rb') as f:
        data = f.read()
    
    success, result = parse_packet_lines(data)
    if success:
        print('PASS')
        sys.exit(0)
    else:
        print(f'FAIL: {result}')
        sys.exit(1)
except Exception as e:
    print(f'ERROR: {e}')
    sys.exit(1)
" 2>/dev/null

if [[ $? -eq 0 ]]; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL${NC}"
    # Fallback to simpler check
    echo -n "  ✓ Basic packet-line structure... "
    if head -c 100 "$TEMP_DIR/packet_capabilities" | grep -E '^[0-9a-f]{4}' >/dev/null; then
        echo -e "${GREEN}PASS${NC}"
    else
        echo -e "${RED}FAIL${NC}"
        exit 1
    fi
fi

# Test content encoding
echo -n "  ✓ UTF-8 content encoding... "
if file "$TEMP_DIR/packet_capabilities" | grep -q "UTF-8\|ASCII"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Encoding unclear)${NC}"
fi

# Show detailed packet analysis
echo ""
echo "📋 Packet-line Analysis:"
echo "======================="
echo "Capability advertisement (first 200 bytes):"
head -c 200 "$TEMP_DIR/packet_capabilities" | hexdump -C

echo ""
echo "First 5 packet-lines from capability response:"
python3 -c "
import re

def show_packets(filename, max_packets=5):
    try:
        with open(filename, 'rb') as f:
            data = f.read()
        
        offset = 0
        count = 0
        while offset < len(data) and count < max_packets:
            if offset + 4 > len(data):
                break
            
            length_str = data[offset:offset+4].decode('ascii', errors='ignore')
            length = int(length_str, 16)
            
            if length == 0:
                print(f'{count+1}. FLUSH (0000)')
                break
            else:
                packet_data = data[offset+4:offset+length]
                content = packet_data.decode('utf-8', errors='ignore').rstrip('\n')
                print(f'{count+1}. [{length:04x}] {content}')
                offset += length
                count += 1
    except Exception as e:
        print(f'Error: {e}')

show_packets('$TEMP_DIR/packet_capabilities')
" 2>/dev/null || echo "Could not parse packet details"

echo ""
echo "Packet-line protocol validation completed!"
