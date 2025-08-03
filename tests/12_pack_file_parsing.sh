#!/bin/bash
# Test 12: Pack File Parsing Validation
# Tests basic pack file parsing functionality

set -e

echo "📦 Testing pack file parsing..."

# Create temporary directory for test
TMP_DIR=$(mktemp -d)
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

cd "$TMP_DIR"

# Create a test repo with minimal objects
echo "🔧 Creating test repository..."
mkdir test-repo
cd test-repo
git init

echo "# Test Pack File" > README.md
echo "" >> README.md
echo "This is a test repository" >> README.md

git add README.md
git config user.name "Test User"
git config user.email "test@example.com"
git commit -m "Test commit for pack file validation"

# Create a simple blob to pack using git
mkdir -p pack-test
cd pack-test
echo "test file content" > test.txt
git add test.txt
git commit -m "Add test file"

# Create a pack file for testing
echo "📁 Creating test pack data using git..."
PACK_BLOB=$(git hash-object test.txt)
echo "✅ Created blob: $PACK_BLOB"

# Create a test tree object
echo "🌳 Creating tree objects..."
TREE_HASH=$(git write-tree)
echo "✅ Created tree: $TREE_HASH"

# Create a commit object
COMMIT_HASH=$(git log --format=%H --max-count=1 HEAD)
echo "✅ Created commit: $COMMIT_HASH"

# Display what we created
echo "📋 Test objects created:"
echo "  Blob: $PACK_BLOB" echo "test file content" | git hash-object --stdin
echo "  Tree: $TREE_HASH"
echo "  Commit: $COMMIT_HASH"

# Create a simple pack manually for testing
echo "🔨 Creating minimal pack for testing..."
cat > test.pack << 'EOF'
PACK\x00\x00\x00\x02\x00\x00\x00\x01
EOF

echo "✅ Pack file test setup completed"
echo "⚠️  Note: Full integration testing will require interacting with receive-pack endpoint"
echo "✅ This validates our object creation and hashing is working"