#!/bin/bash

echo "🔍 TESTING CURRENT PROTOCOL FORMAT"
echo "=================================="
echo

echo "Testing receive-pack response format..."
echo "Raw output:"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack" | xxd | head -5
echo

echo "Human-readable:"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack"
echo

echo "Quick push test..."
rm -rf quick-test
mkdir quick-test
cd quick-test

git init
git config user.email "test@example.com"
git config user.name "Test User"
echo "test" > file.txt
git add file.txt
git commit -m "Test"
git remote add origin http://localhost:8080

echo "Attempting push..."
timeout 10 git push origin main 2>&1 || echo "Push timed out or failed"

cd ..
rm -rf quick-test
