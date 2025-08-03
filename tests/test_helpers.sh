#!/bin/bash

# Test helper functions implementing the new style guide
# Source this file in test scripts: source "$(dirname "$0")/test_helpers.sh"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# Parse Git packet-line data into readable format
parse_packet_line() {
    local data="$1"
    local result=""
    local pos=0
    local length=${#data}
    
    while [[ $pos -lt $length ]]; do
        # Extract 4-character length prefix
        if [[ $((pos + 4)) -gt $length ]]; then
            result="${result}[truncated at position $pos]\n"
            break
        fi
        
        local len_hex="${data:$pos:4}"
        local packet_len
        
        # Convert hex to decimal
        if ! packet_len=$(printf "%d" "0x$len_hex" 2>/dev/null); then
            result="${result}${len_hex} -> [invalid hex]\n"
            break
        fi
        
        # Handle special cases
        if [[ $packet_len -eq 0 ]]; then
            result="${result}0000 -> [flush packet]\n"
            pos=$((pos + 4))
            continue
        fi
        
        if [[ $packet_len -lt 4 ]]; then
            result="${result}${len_hex} -> [invalid length: $packet_len]\n"
            break
        fi
        
        # Extract packet content
        local content_len=$((packet_len - 4))
        local next_pos=$((pos + packet_len))
        
        if [[ $next_pos -gt $length ]]; then
            result="${result}${len_hex} -> [packet extends beyond data: ${packet_len} bytes at offset $pos]\n"
            break
        fi
        
        local content="${data:$((pos + 4)):$content_len}"
        
        # Try to make content readable
        if [[ "$content" =~ ^[[:print:][:space:]]*$ ]]; then
            result="${result}${len_hex} -> \"$content\"\n"
        else
            local hex_content=$(echo -n "$content" | xxd -p | tr -d '\n')
            result="${result}${len_hex} -> [binary: $hex_content]\n"
        fi
        
        pos=$next_pos
    done
    
    echo -e "$result"
}

# Show test success (minimal output)
test_pass() {
    local test_name="$1"
    echo "✓ ${test_name}"
}

# Show test failure (full context)
test_fail() {
    local test_name="$1"
    local expected="$2"
    local actual="$3"
    local raw_data="$4"
    local issue="$5"
    local check_location="$6"
    
    echo "✗ ${test_name}"
    
    if [[ -n "$expected" ]]; then
        echo "  Expected: $expected"
    fi
    
    if [[ -n "$actual" ]]; then
        echo "  Actual: $actual"
    fi
    
    # Parse packet-line data if it looks like hex
    if [[ -n "$raw_data" && "$raw_data" =~ ^[0-9a-f]+$ ]]; then
        echo "  Parsed packet-line data:"
        parse_packet_line "$raw_data" | sed 's/^/    /'
    elif [[ -n "$raw_data" ]]; then
        echo "  Raw response: $raw_data"
    fi
    
    if [[ -n "$issue" ]]; then
        echo "  Issue: $issue"
    fi
    
    if [[ -n "$check_location" ]]; then
        echo "  Check: $check_location"
    fi
}

# Check HTTP response and extract status code
check_http_response() {
    local url="$1"
    local output_file="$2"
    local expected_status="${3:-200}"
    
    local response
    if ! response=$(curl -s -w "%{http_code}" -o "$output_file" "$url" 2>&1); then
        echo "connection_error"
        return 1
    fi
    
    local status_code="${response: -3}"
    if [[ "$status_code" != "$expected_status" ]]; then
        echo "$status_code"
        return 1
    fi
    
    echo "success"
    return 0
}

# Check if content contains expected string
check_content_contains() {
    local file="$1"
    local expected="$2"
    local case_sensitive="${3:-true}"
    
    if [[ "$case_sensitive" == "true" ]]; then
        grep -q "$expected" "$file"
    else
        grep -qi "$expected" "$file"
    fi
}

# Validate Git packet-line format
validate_packet_format() {
    local data="$1"
    
    # Check if starts with 4 hex characters
    if [[ ! "$data" =~ ^[0-9a-f]{4} ]]; then
        echo "invalid_header"
        return 1
    fi
    
    local len_hex="${data:0:4}"
    local packet_len
    
    if ! packet_len=$(printf "%d" "0x$len_hex" 2>/dev/null); then
        echo "invalid_length_hex"
        return 1
    fi
    
    if [[ $packet_len -gt 0 && $packet_len -lt 4 ]]; then
        echo "invalid_length_value"
        return 1
    fi
    
    echo "valid"
    return 0
}

# Get readable test name from script name
get_test_name() {
    local script_path="$1"
    local basename=$(basename "$script_path" .sh)
    echo "${basename}"
}

# Check if server is responding
check_server_health() {
    local server_url="$1"
    local temp_file="$2"
    
    if check_http_response "$server_url/" "$temp_file" "200" >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Send POST request with packet-line data
send_git_request() {
    local url="$1"
    local data="$2"
    local output_file="$3"
    
    printf "%s" "$data" | curl -s -w "%{http_code}" \
        -X POST \
        -H "Content-Type: application/x-git-upload-pack-request" \
        --data-binary @- \
        -o "$output_file" \
        "$url"
}

# Format a Git packet-line (length + content)
format_packet() {
    local content="$1"
    local length=$((${#content} + 4))
    printf "%04x%s" "$length" "$content"
}

# Create ls-refs request
create_ls_refs_request() {
    # command=ls-refs + newline
    local cmd_packet=$(format_packet "command=ls-refs"$'\n')
    local flush_packet="0000"
    printf "%s%s" "$cmd_packet" "$flush_packet"
}
