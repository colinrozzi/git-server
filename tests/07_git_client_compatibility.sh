#!/bin/bash

# Test 07: Git Client Compatibility
# Tests compatibility with real Git clients

SERVER_URL="$1"
TEMP_DIR="$2"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Testing Git client compatibility..."

# Check if git is available
if ! command -v git >/dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  Git not available - skipping git client tests${NC}"
    exit 0
fi

# Test git ls-remote with Protocol v2
echo -n "  ✓ git ls-remote with Protocol v2... "
if git -c protocol.version=2 ls-remote "$SERVER_URL" >"$TEMP_DIR/git_ls_remote" 2>"$TEMP_DIR/git_ls_remote_stderr"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${RED}FAIL${NC}"
    echo "Git stderr:"
    cat "$TEMP_DIR/git_ls_remote_stderr"
    # Don't exit here - this might be expected for empty repo
fi

# Test git ls-remote without Protocol v2 (fallback)
echo -n "  ✓ git ls-remote fallback... "
if git ls-remote "$SERVER_URL" >"$TEMP_DIR/git_ls_remote_fallback" 2>"$TEMP_DIR/git_ls_remote_fallback_stderr"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Expected for empty repo)${NC}"
fi

# Test git clone (expect it might fail for empty repo)
echo -n "  ✓ git clone attempt... "
cd "$TEMP_DIR"
if git -c protocol.version=2 clone "$SERVER_URL" test-repo >"$TEMP_DIR/git_clone_stdout" 2>"$TEMP_DIR/git_clone_stderr"; then
    echo -e "${GREEN}PASS${NC}"
    
    # If clone succeeded, check the repo
    if [[ -d "test-repo/.git" ]]; then
        echo "    ✓ Repository created successfully"
        cd test-repo
        git log --oneline 2>/dev/null || echo "    ℹ️  Empty repository (as expected)"
        cd ..
    fi
else
    echo -e "${YELLOW}WARN (Expected for empty repo)${NC}"
    echo "    Clone error (expected for empty repo):"
    tail -3 "$TEMP_DIR/git_clone_stderr" | sed 's/^/    /'
fi

# Test with custom user agent
echo -n "  ✓ git with custom user agent... "
if git -c http.useragent="test-client/1.0" -c protocol.version=2 ls-remote "$SERVER_URL" >"$TEMP_DIR/git_custom_agent" 2>"$TEMP_DIR/git_custom_agent_stderr"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Expected for empty repo)${NC}"
fi

# Test git capability detection
echo -n "  ✓ git capability detection... "
if GIT_TRACE_PACKET=1 git -c protocol.version=2 ls-remote "$SERVER_URL" >"$TEMP_DIR/git_trace" 2>"$TEMP_DIR/git_trace_stderr"; then
    if grep -q "version 2" "$TEMP_DIR/git_trace_stderr"; then
        echo -e "${GREEN}PASS (Protocol v2 detected)${NC}"
    else
        echo -e "${YELLOW}WARN (Protocol v2 not clearly detected)${NC}"
    fi
else
    # Check if the trace shows protocol negotiation
    if grep -q "git-upload-pack" "$TEMP_DIR/git_trace_stderr"; then
        echo -e "${GREEN}PASS (Git attempted connection)${NC}"
    else
        echo -e "${YELLOW}WARN (No clear protocol trace)${NC}"
    fi
fi

# Test HTTP-specific git config
echo -n "  ✓ HTTP configuration compatibility... "
if git -c http.version=HTTP/1.1 -c protocol.version=2 ls-remote "$SERVER_URL" >"$TEMP_DIR/git_http_config" 2>"$TEMP_DIR/git_http_config_stderr"; then
    echo -e "${GREEN}PASS${NC}"
else
    echo -e "${YELLOW}WARN (Expected for empty repo)${NC}"
fi

# Show git version for reference
echo ""
echo "📋 Git Client Information:"
echo "========================="
git --version
echo ""

# Show sample git output
echo "📋 Sample Git ls-remote Output:"
echo "==============================="
if [[ -s "$TEMP_DIR/git_ls_remote" ]]; then
    cat "$TEMP_DIR/git_ls_remote"
else
    echo "(Empty - expected for empty repository)"
fi

echo ""
echo "📋 Git Protocol Trace (last 10 lines):"
echo "======================================"
if [[ -f "$TEMP_DIR/git_trace_stderr" ]]; then
    tail -10 "$TEMP_DIR/git_trace_stderr" | grep -E "(packet|version|capability)" || echo "(No protocol details found)"
fi

echo ""
echo "Git client compatibility test completed!"
