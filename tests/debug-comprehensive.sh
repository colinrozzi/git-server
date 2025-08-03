#!/bin/bash

echo "🔧 COMPREHENSIVE GIT SERVER DEBUG"
echo "=================================="
echo

# Test 1: Basic server connectivity
echo "1️⃣ Testing basic server connectivity..."
response=$(curl -s -w "%{http_code}" http://localhost:8080/)
echo "Response code: ${response: -3}"
echo "First few lines of response:"
echo "$response" | head -5
echo

# Test 2: Upload-pack info/refs (Protocol v2)
echo "2️⃣ Testing upload-pack info/refs (Protocol v2)..."
echo "URL: http://localhost:8080/info/refs?service=git-upload-pack"
upload_response=$(curl -s "http://localhost:8080/info/refs?service=git-upload-pack")
echo "Upload-pack response:"
echo "$upload_response" | head -10
echo

# Test 3: Receive-pack info/refs (Protocol v1)  
echo "3️⃣ Testing receive-pack info/refs (Protocol v1)..."
echo "URL: http://localhost:8080/info/refs?service=git-receive-pack"
receive_response=$(curl -s "http://localhost:8080/info/refs?service=git-receive-pack")
echo "Receive-pack response:"
echo "$receive_response" | head -10
echo

# Test 4: Git client ls-remote test
echo "4️⃣ Testing Git client ls-remote..."
echo "Command: git ls-remote http://localhost:8080"
git ls-remote http://localhost:8080 2>&1 | head -10
echo

# Test 5: Git client with Protocol v2 explicit
echo "5️⃣ Testing Git client with explicit Protocol v2..."
echo "Command: git -c protocol.version=2 ls-remote http://localhost:8080"
git -c protocol.version=2 ls-remote http://localhost:8080 2>&1 | head -10
echo

# Test 6: Check what the Theater server logs show
echo "6️⃣ Current server state..."
echo "Refs endpoint:"
curl -s http://localhost:8080/refs
echo
echo "Objects endpoint:"
curl -s http://localhost:8080/objects
echo

echo "🎯 Summary:"
echo "- Server is $([ "${response: -3}" = "200" ] && echo "✅ responding" || echo "❌ not responding")"
echo "- Upload-pack: $([ -n "$upload_response" ] && echo "✅ has response" || echo "❌ no response")"  
echo "- Receive-pack: $([ -n "$receive_response" ] && echo "✅ has response" || echo "❌ no response")"
