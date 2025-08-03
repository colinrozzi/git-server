#!/bin/bash

echo "=== Quick Push Test (no sideband) ==="

# Clean up
rm -rf quick_test_repo
mkdir quick_test_repo
cd quick_test_repo

# Create simple repo
git init
echo "test content" > test.txt
git add test.txt
git config user.email "test@example.com" 
git config user.name "Test User"
git commit -m "Test commit"

echo "Attempting push..."
git remote add origin http://localhost:8080
git push origin main 2>&1

echo "Push exit code: $?"
