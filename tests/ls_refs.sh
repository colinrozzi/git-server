#!/bin/bash

# Test: ls-refs Protocol v2 command
# Verifies basic ls-refs functionality for reference listing

SERVER_URL="$1"
TEMP_DIR="$2"

# Source helper functions
source "$(dirname "$0")/test_helpers.sh"

# Test basic ls-refs command
request_data=$(create_ls_refs_request)
response_code=$(send_git_request "$SERVER_URL/git-upload-pack" "$request_data" "$TEMP_DIR/ls_refs_response")

if [[ "$response_code" != "200" ]]; then
    test_fail "ls_refs_http_response" \
              "HTTP 200" \
              "HTTP $response_code" \
              "$(cat "$TEMP_DIR/ls_refs_response" 2>/dev/null || echo "No response")" \
              "ls-refs command rejected by server" \
              "Check handle_upload_pack_request() routing in protocol/http.rs"
    exit 1
fi

# Test response format (should end with flush packet for empty repo)
response_content=$(cat "$TEMP_DIR/ls_refs_response")

# For empty repository, expect just a flush packet (0000)
if [[ "$response_content" == "0000" ]]; then
    # Perfect - empty repo returns flush packet only
    exit 0
elif [[ "$response_content" =~ 0000$ ]]; then
    # Has refs and ends with flush packet - good
    exit 0
else
    test_fail "ls_refs_response_format" \
              "Response ending with flush packet (0000)" \
              "$(parse_packet_line "$response_content" | tail -1)" \
              "$response_content" \
              "ls-refs response not properly terminated" \
              "Check ls-refs response formatting in handle_ls_refs_command()"
    exit 1
fi

# All ls-refs tests passed
exit 0
