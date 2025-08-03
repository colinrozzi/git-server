#!/bin/bash
# Fixed test script for Git Protocol v2

echo "🚀 Testing Git push with Protocol v2..."

# Clean up any existing test directory
rm -rf tmp-dir

mkdir tmp-dir
cd tmp-dir

# Initialize repository
echo "📝 Initializing Git repository..."
git init
git config user.email "test@example.com"
git config user.name "Test User"

# Create and commit file
echo "test content" > test.txt
git add test.txt
git commit -m "Initial commit"

echo "📋 Current branch:"
git branch

echo "📋 Current HEAD:"
git log --oneline -1

# Add remote
git remote add origin http://localhost:8080

echo "🔍 Testing server connectivity first..."
curl -s http://localhost:8080/ | head -5

echo
echo "🔍 Testing Git Protocol v2 capability advertisement..."
curl -s "http://localhost:8080/info/refs?service=git-upload-pack" | head -10

echo
echo "🚀 Attempting push with explicit Protocol v2..."
echo "Current branch: $(git branch --show-current)"

# Try pushing the current branch (should be main, not master)
CURRENT_BRANCH=$(git branch --show-current)
echo "Pushing branch: $CURRENT_BRANCH"

# Push with verbose output and Protocol v2
GIT_TRACE=1 GIT_CURL_VERBOSE=1 git -c protocol.version=2 push origin "$CURRENT_BRANCH" 2>&1

cd ..
rm -rf tmp-dir
