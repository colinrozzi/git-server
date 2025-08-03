#!/bin/bash

echo "=== Test With NUL Byte Fix ==="

# Clean start  
rm -rf nul_fix_test
mkdir nul_fix_test
cd nul_fix_test

# Create test repo
git init > /dev/null 2>&1
echo "nul byte fix test" > fix.txt
git add fix.txt
git config user.email "fix@test.com"
git config user.name "Fix User"
git commit -m "NUL byte fix test" > /dev/null 2>&1

echo "Testing push with NUL byte fix..."
git remote add origin http://localhost:8080
git push origin main

echo -e "\nPush result: $?"

if [ $? -eq 0 ]; then
    echo "🎉 SUCCESS! Push worked!"
    echo "Let's verify with ls-remote:"
    git ls-remote origin
else
    echo "❌ Still failing. Check logs."
fi
