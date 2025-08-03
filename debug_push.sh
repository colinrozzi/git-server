#!/bin/bash

echo "=== Debug Test with Logging ==="

# Start clean
rm -rf debug_push_test
mkdir debug_push_test
cd debug_push_test

# Create simple repo
git init > /dev/null 2>&1
echo "debug content" > debug.txt
git add debug.txt
git config user.email "debug@test.com"
git config user.name "Debug User"
git commit -m "Debug push" > /dev/null 2>&1

echo "Created test repo. Attempting push..."
git remote add origin http://localhost:8080

# Simple push
git push origin main 2>&1

echo -e "\nDone. Check the server logs for debug output!"
