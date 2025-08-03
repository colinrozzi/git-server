#!/bin/bash

echo "🔍 DEBUG: What's actually being returned by receive-pack..."
echo

# Raw curl to see exact bytes
echo "Raw receive-pack response:"
curl -s "http://localhost:8080/info/refs?service=git-receive-pack" | xxd | head -20

echo
echo "For comparison, upload-pack response:"
curl -s "http://localhost:8080/info/refs?service=git-upload-pack" | xxd | head -20

echo
echo "🔍 Let's see if the server logs show which function is being called..."
echo "Make a request and check Theater logs to see which log message appears:"
echo "Expected: 'Generating Protocol v1 capability advertisement for receive-pack (push compatibility)'"

curl -s "http://localhost:8080/info/refs?service=git-receive-pack" > /dev/null

echo
echo "✅ Request sent - check Theater logs with 'th events [actor-id]'"
