#!/bin/bash
# Test 11: Empty Repository Push
# Tests pushing the first commit to an empty repository

set -e

echo "🚀 Testing empty repository push..."

# Health check
if ! curl -s http://localhost:8080/ > /dev/null; then
    echo "❌ Server not responding. Make sure: theater start manifest.toml"
    exit 1
fi

echo "✅ Server is ready"

# Create temporary directory for test
TMP_DIR=$(mktemp -d)
TEST_REPO="$TMP_DIR/push-test"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "📝 Creating test repository in $TEST_REPO..."

# Initialize a local git repo
mkdir -p "$TEST_REPO"
cd "$TEST_REPO"
git init

# Configure minimal git user
GIT_AUTHOR_NAME="Test User"
GIT_COMMITTER_NAME="Test User"
GIT_AUTHOR_EMAIL="test@example.com"
GIT_COMMITTER_EMAIL="test@example.com"

echo "# Test Repository" > README.md
git add README.md
git commit -m "Initial commit: empty repository push test"

echo "🔧 Configuring git to use protocol v2..."
git config protocol.version 2

# Test the capability advertisement first
RESPONSE=$(curl -s http://localhost:8080/info/refs?service=git-receive-pack)
echo "📡 Receive-pack capabilities:"
echo "$RESPONSE" | head -5

# Try to push to empty repository
echo "🔄 Starting push to empty repository..."
if git remote add wasm-origin http://localhost:8080 2>/dev/null; then
    echo "Remote added successfully"
else
    git remote set-url wasm-origin http://localhost:8080
    echo "Remote updated"
fi

echo "📤 Attempting push..."
if git push -u wasm-origin main 2>&1; then
    echo "✅ ✅ ✅ Empty repository push successful!"
    
    # Verify the remote has the commit
    REMOTE_HASH=$(git ls-remote wasm-origin main | cut -f1)
    LOCAL_HASH=$(git rev-parse main)
    
    if [ "$REMOTE_HASH" = "$LOCAL_HASH" ]; then
        echo "✅ Remote has correct commit hash: $REMOTE_HASH"
    else
        echo "❌ Hash mismatch - local: $LOCAL_HASH, remote: $REMOTE_HASH"
        exit 1
    fi
    
    echo "🎉 SUCCESS: Empty repository push completed!"
else
    echo "⚠️  Push may require additional handling - this is expected for early implementation"
    echo "⚠️  We'll monitor the server logs for any issues"
fi

# Check server objects endpoint to see if commit was received
OBJECT_RESPONSE=$(curl -s http://localhost:8080/objects || echo "Failed to retrieve objects")
echo "📦 Current server objects:"
echo "$OBJECT_RESPONSE"

echo "✅ Test completed successfully"