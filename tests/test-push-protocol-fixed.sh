#!/bin/bash

echo "🚀 TESTING GIT PUSH - PROTOCOL FIXED VERSION"
echo "============================================"
echo

# Clean up
rm -rf tmp-push-test

echo "📝 Setting up test repository..."
mkdir tmp-push-test
cd tmp-push-test

# Initialize repo
git init
git config user.email "test@example.com"
git config user.name "Test User"

# Create test content
echo "Hello, Git Server!" > README.md
git add README.md
git commit -m "Initial commit"

echo "📋 Repository state:"
echo "Branch: $(git branch --show-current)"
echo "Commit: $(git log --oneline -1)"
echo

# Add remote
git remote add origin http://localhost:8080

echo "🔍 Testing server connectivity..."
curl -s http://localhost:8080/ | head -3
echo

echo "🔍 Testing receive-pack capability advertisement..."
curl -s "http://localhost:8080/info/refs?service=git-receive-pack" | head -2
echo
echo

echo "🚀 Attempting Git push..."
echo "Command: git push origin main"
echo

# Push with verbose output
GIT_TRACE=1 GIT_CURL_VERBOSE=1 git push origin main 2>&1

echo
echo "📊 Push result: $?"
echo

echo "🔍 Checking server state after push..."
curl -s http://localhost:8080/refs
echo
curl -s http://localhost:8080/objects | head -10

cd ..
rm -rf tmp-push-test

echo
echo "🎯 Test completed!"
