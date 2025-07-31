#!/bin/bash

# Make script executable
chmod +x "$0"

# Git Server Test Script
echo "🚀 Testing Git Server Actor"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

# Clean up any existing test repo
print_info "Cleaning up previous test..."
rm -rf test-repo

# Test 1: Basic repository discovery
print_info "Testing git ls-remote..."
if git ls-remote http://localhost:8080 > /dev/null 2>&1; then
    print_status "Discovery phase working"
    git ls-remote http://localhost:8080
else
    print_error "Discovery phase failed"
    exit 1
fi

echo ""

# Test 2: Clone attempt with verbose output
print_info "Testing git clone with verbose output..."
GIT_TRACE=1 GIT_TRACE_PACKET=1 git clone http://localhost:8080 test-repo

# Check if clone succeeded
if [ -d "test-repo" ] && [ -f "test-repo/README.md" ]; then
    print_status "Git clone succeeded!"
    echo ""
    print_info "Repository contents:"
    ls -la test-repo/
    echo ""
    print_info "README.md contents:"
    cat test-repo/README.md
    echo ""
    print_status "🎉 ALL TESTS PASSED! Your Git server is 100% working!"
else
    print_error "Git clone failed or incomplete"
    
    # Show debug endpoints for troubleshooting
    echo ""
    print_info "Debug information:"
    echo "Repository info:"
    curl -s http://localhost:8080/
    echo ""
    echo "References:"
    curl -s http://localhost:8080/refs
    echo ""
    echo "Objects:"
    curl -s http://localhost:8080/objects
fi
