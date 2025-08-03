#!/bin/bash
# Focused Git Server Test Runner

set -e

SERVER_URL="http://localhost:8080"
TESTS_DIR="$(dirname "$0")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
PASSED=0
FAILED=0

log_info() {
    echo "$1"
}

log_success() {
    echo -e "${GREEN}$1${NC}"
}

log_error() {
    echo -e "${RED}$1${NC}"
}

log_warning() {
    echo -e "${YELLOW}$1${NC}"
}

# Quick server check
check_server() {
    if curl -s "$SERVER_URL" > /dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Run a single test with focused output
run_test() {
    local test_name="$1"
    local test_file="$TESTS_DIR/${test_name}.sh"
    
    if [ ! -f "$test_file" ]; then
        log_error "Test file not found: $test_file"
        return 1
    fi
    
    echo -n "Running $test_name... "
    
    # Capture output and exit code
    if OUTPUT=$(bash "$test_file" 2>&1); then
        log_success "PASS"
        ((PASSED++))
        
        # Only show output if it contains specific markers
        if echo "$OUTPUT" | grep -q "✗\|FAIL\|ERROR"; then
            echo "$OUTPUT" | grep "✗\|FAIL\|ERROR"
        fi
    else
        log_error "FAIL"
        ((FAILED++))
        
        # Show relevant error lines
        echo "$OUTPUT" | grep -E "(✗|FAIL|ERROR|fatal:|error:)" | head -3
    fi
}

# Main execution
main() {
    log_info "Git Server Focused Test Suite"
    log_info "=============================="
    
    # Server check
    if ! check_server; then
        log_error "Server not responding at $SERVER_URL"
        log_info "Start server with: theater start manifest.toml"
        exit 1
    fi
    
    log_success "Server is responding"
    echo ""
    
    # Run specific test or all tests
    if [ $# -gt 0 ]; then
        for test in "$@"; do
            run_test "$test"
        done
    else
        # Run all numbered tests
        for test_file in "$TESTS_DIR"/[0-9][0-9]_*.sh; do
            if [ -f "$test_file" ]; then
                test_name=$(basename "$test_file" .sh)
                run_test "$test_name"
            fi
        done
    fi
    
    echo ""
    log_info "Summary: $PASSED passed, $FAILED failed"
    
    if [ $FAILED -gt 0 ]; then
        exit 1
    fi
}

main "$@"