#!/bin/bash

# Test: git push to empty repository
# Verifies that a real git client can push to the server

SERVER_URL="$1"
TEMP_DIR="$2"

# Source helper functions
source "$(dirname "$0")/test_helpers.sh"

# Create a test repository to push from
TEST_REPO="$TEMP_DIR/test_repo"
rm -rf "$TEST_REPO"
mkdir -p "$TEST_REPO"
cd "$TEST_REPO"

# Initialize and create a simple commit
if ! git init --quiet 2>/dev/null; then
    test_fail "git_init" \
              "Git repository initialization" \
              "git init failed" \
              "" \
              "Cannot create test repository" \
              "Check git installation"
    exit 1
fi

# Configure git for the test
git config user.name "Test User"
git config user.email "test@example.com"
git config protocol.version 2

# Create initial content
echo "# Test Repository" > README.md
git add README.md

if ! git commit -m "Initial commit" --quiet 2>/dev/null; then
    test_fail "git_commit" \
              "Initial commit creation" \
              "git commit failed" \
              "" \
              "Cannot create test commit" \
              "Check git configuration"
    exit 1
fi

# Add remote and attempt push
git remote add origin "$SERVER_URL"

# Capture git push output
local push_output="$TEMP_DIR/push_output"
local push_error="$TEMP_DIR/push_error"

if git push origin main >"$push_output" 2>"$push_error"; then
    # Success! Check that the push actually worked by verifying server state
    # This would require checking /refs endpoint or similar
    exit 0
else
    # Parse the error to understand what went wrong
    local error_msg=$(cat "$push_error")
    
    # Common failure patterns
    if [[ "$error_msg" =~ "invalid server response" ]]; then
        test_fail "git_push_protocol_error" \
                  "Valid Git Protocol v2 response from server" \
                  "Git client reports invalid server response" \
                  "$error_msg" \
                  "Server not implementing Git protocol correctly" \
                  "Check receive-pack implementation or capability advertisement"
    elif [[ "$error_msg" =~ "not implemented" ]]; then
        test_fail "git_push_not_implemented" \
                  "Push operation support" \
                  "Server reports push not implemented" \
                  "$error_msg" \
                  "Push/receive-pack not implemented yet" \
                  "Implement handle_receive_pack_request() in protocol/http.rs"
    elif [[ "$error_msg" =~ "fatal:" ]]; then
        test_fail "git_push_fatal_error" \
                  "Successful push operation" \
                  "Git client fatal error" \
                  "$error_msg" \
                  "Git protocol communication failed" \
                  "Check server logs and Protocol v2 implementation"
    else
        test_fail "git_push_unknown_error" \
                  "Successful push operation" \
                  "Unknown error during push" \
                  "$error_msg" \
                  "Unexpected git push failure" \
                  "Review full error output and server implementation"
    fi
    exit 1
fi
