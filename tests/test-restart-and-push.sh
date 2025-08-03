#!/bin/bash

echo "🔄 RESTARTING THEATER AND TESTING THE FIX"
echo "=========================================="
echo

echo "1️⃣ Stopping current Theater instance..."
theater stop 2>/dev/null || echo "No instance running"

echo
echo "2️⃣ Starting fresh Theater instance..."
theater start manifest.toml &
SERVER_PID=$!

echo "Waiting for server to start..."
sleep 3

echo
echo "3️⃣ Testing the fixed Protocol v1 format..."
echo "Receive-pack capability advertisement:"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack" | head -2
echo

echo
echo "4️⃣ Testing Git push with the fix..."
rm -rf tmp-test-fix
mkdir tmp-test-fix
cd tmp-test-fix

git init
git config user.email "test@example.com" 
git config user.name "Test User"

echo "Hello, fixed Git server!" > test.txt
git add test.txt
git commit -m "Test commit with fixed protocol"

git remote add origin http://localhost:8080

echo "Pushing to server..."
git push origin main 2>&1

echo
echo "📊 Checking server state after push..."
curl -s http://localhost:8080/refs
echo
curl -s http://localhost:8080/objects | head -5

cd ..
rm -rf tmp-test-fix

echo
echo "✅ Test completed!"
