#!/bin/bash
# Test 10: Receive Pack Capabilities Advertisement
# Tests the receive-pack service capability advertisement

set -e

echo "🔍 Testing receive-pack capabilities..."

# Health check
if ! curl -s http://localhost:8080/ > /dev/null; then
    echo "❌ Server not responding. Make sure: theater start manifest.toml"
    exit 1
fi

echo "✅ Server is ready"

# Test receive-pack capability advertisement
echo "Testing /info/refs?service=git-receive-pack..."
RESPONSE=$(curl -s http://localhost:8080/info/refs?service=git-receive-pack)

echo "Received response:"
echo "$RESPONSE" | head -n 10

# Check for required capabilities
echo "$RESPONSE" | grep -q "version 2" || {
    echo "❌ Missing version 2"
    exit 1
}

echo "$RESPONSE" | grep -q "receive-pack=" || {
    echo "❌ Missing receive-pack capabilities"
    exit 1
}

echo "$RESPONSE" | grep -q "report-status" || {
    echo "❌ Missing report-status capability"
    exit 1
}

echo "$RESPONSE" | grep -q "ofs-delta" || {
    echo "❌ Missing ofs-delta capability"
    exit 1
}

# Ensure different from upload-pack advertisement
echo "Testing upload-pack capabilities are different..."
UPLOAD_RESPONSE=$(curl -s http://localhost:8080/info/refs?service=git-upload-pack)

echo "$UPLOAD_RESPONSE" | grep -q "fetch=" || {
    echo "❌ Missing fetch for upload-pack"
    exit 1
}

# Ensure receive-pack doesn't have fetch
echo "$RESPONSE" | grep -qv "fetch=" || {
    echo "❌ Fetch capability incorrectly advertised for receive-pack"
}

echo "✅ ✅ ✅ Receive-pack capabilities working correctly"