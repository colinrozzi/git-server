#!/bin/bash
# Test 11: Empty Repository Push (Focused Version)

set -e

# Silent health check
if ! curl -s http://localhost:8080/ > /dev/null 2>&1; then
    echo "FAIL: Server not responding"
    exit 1
fi

# Setup temp repo
TMP_DIR=$(mktemp -d)
TEST_REPO="$TMP_DIR/push-test"
trap "rm -rf $TMP_DIR" EXIT

# Create test commit (silent)
mkdir -p "$TEST_REPO"
cd "$TEST_REPO"
git init -q
git config user.name "Test User"
git config user.email "test@example.com"
echo "# Test Repository" > README.md
git add README.md
git commit -q -m "Initial commit"
git config protocol.version 2

# Test capability advertisement
echo "Testing receive-pack capabilities..."
RESPONSE=$(curl -s "http://localhost:8080/info/refs?service=git-receive-pack")

# Check for protocol v2 markers (without printing binary data)
if echo "$RESPONSE" | grep -q "version 2"; then
    echo "✓ Protocol v2 detected"
else
    echo "✗ Protocol v2 missing"
    echo "Raw response length: ${#RESPONSE} bytes"
    # Only show first few readable characters
    echo "Response start: $(echo "$RESPONSE" | head -c 50 | tr -cd '[:print:]' | head -c 20)..."
    exit 1
fi

# Test the actual push
echo "Testing push operation..."
git remote add origin http://localhost:8080

if git push -u origin main 2>&1; then
    echo "✓ Push succeeded"
    
    # Verify push worked
    REMOTE_HASH=$(git ls-remote origin main 2>/dev/null | cut -f1)
    LOCAL_HASH=$(git rev-parse main)
    
    if [ "$REMOTE_HASH" = "$LOCAL_HASH" ]; then
        echo "✓ Hash verification passed"
        echo "PASS: Empty repository push test"
    else
        echo "✗ Hash mismatch (local: ${LOCAL_HASH:0:8}, remote: ${REMOTE_HASH:0:8})"
        echo "FAIL: Hash verification failed"
        exit 1
    fi
else
    PUSH_ERROR=$?
    echo "✗ Push failed with exit code $PUSH_ERROR"
    echo "FAIL: Push operation failed"
    exit 1
fi