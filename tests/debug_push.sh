#!/bin/bash
# Debug Push Test - Focused on the exact failure

set -e

echo "🔍 Debug: Empty Repository Push"
echo "================================"

# Setup
TMP_DIR=$(mktemp -d)
TEST_REPO="$TMP_DIR/test"
trap "rm -rf $TMP_DIR" EXIT

mkdir -p "$TEST_REPO"
cd "$TEST_REPO"
git init -q
git config user.name "Test" && git config user.email "test@test.com"
echo "test" > file.txt
git add file.txt && git commit -q -m "test"
git config protocol.version 2

# Test 1: Capability Advertisement
echo "1. Testing capability advertisement..."
CAPS_RESPONSE=$(curl -s "http://localhost:8080/info/refs?service=git-receive-pack")
CAPS_LENGTH=${#CAPS_RESPONSE}

echo "   Response length: $CAPS_LENGTH bytes"

# Analyze the response format
if [ $CAPS_LENGTH -gt 0 ]; then
    # Show first 20 bytes as hex
    echo -n "   First 20 bytes (hex): "
    echo "$CAPS_RESPONSE" | head -c 20 | hexdump -v -e '/1 "%02x"'
    echo ""
    
    # Try to find version 2
    if echo "$CAPS_RESPONSE" | grep -q "version 2"; then
        echo "   ✓ Contains 'version 2'"
    else
        echo "   ✗ Missing 'version 2'"
    fi
    
    # Check for proper packet line format (first 4 chars should be hex length)
    FIRST_4=$(echo "$CAPS_RESPONSE" | head -c 4)
    if [[ "$FIRST_4" =~ ^[0-9a-fA-F]{4}$ ]]; then
        echo "   ✓ Starts with hex length: $FIRST_4"
        # Convert hex to decimal
        LENGTH_DEC=$((16#$FIRST_4))
        echo "   ✓ First packet length: $LENGTH_DEC bytes"
    else
        echo "   ✗ Invalid packet line format, starts with: '$FIRST_4'"
    fi
else
    echo "   ✗ Empty response"
fi

# Test 2: Push Attempt
echo ""
echo "2. Testing push attempt..."
git remote add origin http://localhost:8080

# Capture the exact error
echo "   Attempting push..."
if PUSH_OUTPUT=$(git push origin main 2>&1); then
    echo "   ✓ Push succeeded unexpectedly"
    echo "$PUSH_OUTPUT"
else
    echo "   ✗ Push failed as expected"
    # Extract the specific error
    echo "$PUSH_OUTPUT" | grep -E "(fatal:|error:)" | head -1
    
    # Look for the specific server response error
    if echo "$PUSH_OUTPUT" | grep -q "invalid server response"; then
        SERVER_RESP=$(echo "$PUSH_OUTPUT" | grep "got '" | sed "s/.*got '\\([^']*\\)'.*/\\1/")
        echo "   Server responded with: '$SERVER_RESP'"
        echo "   Length: ${#SERVER_RESP} characters"
        echo -n "   As hex: "
        echo -n "$SERVER_RESP" | hexdump -v -e '/1 "%02x"'
        echo ""
    fi
fi

echo ""
echo "🔍 Analysis complete"