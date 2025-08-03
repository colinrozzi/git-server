#!/bin/bash

echo "=== Protocol Debug Test ==="

echo "1. Check server status:"
curl -s http://localhost:8080/ && echo " ✅ Server running" || echo " ❌ Server not running"

echo -e "\n2. Check ref advertisement:"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack" | hexdump -C | head -10

echo -e "\n3. Create test repo and attempt push..."
rm -rf debug_repo
mkdir debug_repo
cd debug_repo

git init > /dev/null 2>&1
echo "debug test" > debug.txt
git add debug.txt
git config user.email "debug@test.com"
git config user.name "Debug User"  
git commit -m "Debug commit" > /dev/null 2>&1

echo "Repo created. Attempting push with verbose output..."
git remote add origin http://localhost:8080

# Try push with maximum verbosity
GIT_CURL_VERBOSE=1 GIT_TRACE=1 GIT_TRACE_PACKET=1 git push origin main
