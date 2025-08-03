#!/bin/bash

echo "🔍 Testing server endpoints..."
echo

echo "1. Basic server info:"
curl -s http://localhost:8080/ || echo "❌ Server not responding"
echo
echo

echo "2. Git info/refs (capability advertisement):"
curl -s "http://localhost:8080/info/refs?service=git-upload-pack" || echo "❌ info/refs not working"
echo
echo

echo "3. Refs endpoint:"
curl -s http://localhost:8080/refs || echo "❌ refs endpoint not working"
echo
echo

echo "4. Objects endpoint:"
curl -s http://localhost:8080/objects || echo "❌ objects endpoint not working"
echo
echo

echo "5. Test with Git client (ls-remote):"
echo "This should show Git Protocol v2 in action..."
git -c protocol.version=2 ls-remote http://localhost:8080 2>&1 | head -20
