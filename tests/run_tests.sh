#!/bin/bash

# Make executable
chmod +x "$(dirname "$0")"/test_helpers.sh
chmod +x "$(dirname "$0")"/ls_refs.sh
chmod +x "$(dirname "$0")"/git_push.sh

# Minimal, focused test runner
# Usage: ./run_tests.sh [test_name]

set -e

# Configuration
SERVER_URL="${GIT_SERVER_URL:-http://localhost:8080}"
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMP_DIR="/tmp/git-server-tests"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0

# Source helper functions
source "$TEST_DIR/test_helpers.sh"

# Minimal setup - just check server
setup() {
    mkdir -p "$TEMP_DIR"
    
    if ! check_server_health "$SERVER_URL" "$TEMP_DIR/health_check"; then
        echo -e "${RED}✗ server_not_responding${NC}"
        echo "  Expected: Server responding at $SERVER_URL"
        echo "  Actual: Connection failed"
        echo "  Issue: Git server not running"
        echo "  Check: Run 'theater start manifest.toml' in git-server directory"
        exit 1
    fi
}

# Run a single test with clean output
run_test() {
    local test_script="$1"
    local test_name=$(get_test_name "$test_script")
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    # Run the test script
    if bash "$test_script" "$SERVER_URL" "$TEMP_DIR" 2>&1; then
        test_pass "$test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        # Test already printed failure details
        echo ""  # Add spacing after failure details
    fi
}

# Run essential tests only
run_essential_tests() {
    echo "Running essential git-server tests..."
    echo ""
    
    # Core functionality tests
    local tests=(
        "ls_refs.sh"
        "git_push.sh"
    )
    
    for test in "${tests[@]}"; do
        local test_path="$TEST_DIR/$test"
        if [[ -f "$test_path" ]]; then
            run_test "$test_path"
        else
            echo -e "${RED}✗ missing_test${NC}"
            echo "  Expected: Test file at $test_path"
            echo "  Issue: Test not implemented yet"
        fi
    done
}

# Simple summary
show_summary() {
    echo ""
    local failed=$((TOTAL_TESTS - PASSED_TESTS))
    echo "Summary: $PASSED_TESTS/$TOTAL_TESTS tests passed"
    
    if [[ $failed -eq 0 ]]; then
        exit 0
    else
        exit 1
    fi
}

# Help
show_help() {
    echo "Focused git-server test suite"
    echo ""
    echo "Usage: $0 [test_name]"
    echo ""
    echo "Essential tests:"
    echo "  ls_refs       Test Protocol v2 ls-refs command"
    echo "  git_push      Test git push to empty repository"
    echo ""
    echo "Examples:"
    echo "  $0                # Run all essential tests"
    echo "  $0 ls_refs        # Run specific test"
    echo ""
    echo "Environment:"
    echo "  GIT_SERVER_URL    Server URL (default: http://localhost:8080)"
}

# Main execution
main() {
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        "")
            setup
            run_essential_tests
            show_summary
            ;;
        *)
            # Single test
            setup
            local test_script="$TEST_DIR/$1.sh"
            if [[ -f "$test_script" ]]; then
                run_test "$test_script"
                show_summary
            else
                echo -e "${RED}✗ test_not_found${NC}"
                echo "  Expected: Test file at $test_script"
                echo "  Available tests: ls_refs, git_push"
                exit 1
            fi
            ;;
    esac
}

# Run
main "$@"
