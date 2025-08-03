#!/bin/bash

echo "=== Detailed Protocol Analysis ==="

echo "1. Testing ref advertisement:"
echo "Raw response (hex):"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack" | hexdump -C

echo -e "\n2. Raw response (ASCII):"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack"

echo -e "\n3. Response headers:"
curl -I "http://localhost:8080/info/refs?service=git-receive-pack"

echo -e "\n4. Testing actual push with packet trace..."

# Create minimal test repo
rm -rf trace_test
mkdir trace_test
cd trace_test
git init > /dev/null 2>&1
echo "trace test" > file.txt
git add file.txt
git config user.email "trace@test.com"
git config user.name "Trace User"
git commit -m "Trace commit" > /dev/null 2>&1

# Enable maximum Git protocol tracing
export GIT_TRACE_PACKET=1
export GIT_TRACE_CURL=1

echo "Attempting push with full tracing..."
git remote add origin http://localhost:8080
git push origin main 2>&1 | head -50
