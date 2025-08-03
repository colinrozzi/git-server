#!/bin/bash

echo "=== Test WITHOUT Pack Processing ==="

# Clean start
rm -rf no_pack_test
mkdir no_pack_test
cd no_pack_test

# Create minimal repo
git init > /dev/null 2>&1
echo "no pack test" > test.txt
git add test.txt
git config user.email "no-pack@test.com"
git config user.name "No Pack User"
git commit -m "No pack test" > /dev/null 2>&1

echo "Testing push (pack processing disabled)..."
git remote add origin http://localhost:8080
git push origin main

echo -e "\nPush result: $?"
echo "Check server logs for details!"
