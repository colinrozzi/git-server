#!/bin/bash

# Main test runner for git-server
# Usage: ./run_tests.sh [test_name]

set -e

# Configuration
SERVER_URL="${GIT_SERVER_URL:-http://localhost:8080}"
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMP_DIR="/tmp/git-server-tests"
RESULTS_FILE="$TEMP_DIR/test_results.txt"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Setup
setup() {
    echo -e "${BLUE}🧪 Git Server Test Suite${NC}"
    echo "================================"
    echo "Server URL: $SERVER_URL"
    echo "Test Directory: $TEST_DIR"
    echo "Temp Directory: $TEMP_DIR"
    echo ""
    
    # Create temp directory
    mkdir -p "$TEMP_DIR"
    
    # Clear results file
    echo "# Git Server Test Results - $(date)" > "$RESULTS_FILE"
    
    # Check if server is running
    if ! curl -s "$SERVER_URL/" > /dev/null 2>&1; then
        echo -e "${RED}❌ Server not responding at $SERVER_URL${NC}"
        echo "Make sure your git-server is running with:"
        echo "  theater start manifest.toml"
        exit 1
    fi
    
    echo -e "${GREEN}✅ Server is running${NC}"
    echo ""
}

# Run a single test
run_test() {
    local test_name="$1"
    local test_script="$TEST_DIR/$test_name.sh"
    
    if [[ ! -f "$test_script" ]]; then
        echo -e "${RED}❌ Test not found: $test_name${NC}"
        return 1
    fi
    
    echo -e "${YELLOW}🔧 Running: $test_name${NC}"
    echo "----------------------------------------"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    # Run the test script
    if bash "$test_script" "$SERVER_URL" "$TEMP_DIR"; then
        echo -e "${GREEN}✅ PASSED: $test_name${NC}"
        echo "PASS: $test_name" >> "$RESULTS_FILE"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}❌ FAILED: $test_name${NC}"
        echo "FAIL: $test_name" >> "$RESULTS_FILE"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
    
    echo ""
}

# Run all tests
run_all_tests() {
    local test_files=(
        "01_health_check"
        "02_capability_advertisement"
        "03_ls_refs"
        "04_fetch_command"
        "05_object_info"
        "06_error_handling"
        "07_git_client_compatibility"
        "08_packet_line_validation"
        "09_state_persistence"
    )
    
    for test in "${test_files[@]}"; do
        run_test "$test"
    done
}

# Show summary
show_summary() {
    echo "========================================"
    echo -e "${BLUE}📊 Test Summary${NC}"
    echo "========================================"
    echo "Total Tests: $TOTAL_TESTS"
    echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
    echo -e "Failed: ${RED}$FAILED_TESTS${NC}"
    echo ""
    
    if [[ $FAILED_TESTS -eq 0 ]]; then
        echo -e "${GREEN}🎉 All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}💥 Some tests failed. Check individual test output above.${NC}"
        echo "Detailed results in: $RESULTS_FILE"
        exit 1
    fi
}

# Cleanup
cleanup() {
    echo "Cleaning up temporary files..."
    # Keep test results but clean up other temp files
    find "$TEMP_DIR" -type f ! -name "test_results.txt" -delete 2>/dev/null || true
}

# Help
show_help() {
    echo "Usage: $0 [options] [test_name]"
    echo ""
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -l, --list     List available tests"
    echo "  -c, --clean    Clean up temp directory"
    echo ""
    echo "Examples:"
    echo "  $0                    # Run all tests"
    echo "  $0 01_health_check    # Run specific test"
    echo "  $0 --list             # List available tests"
    echo ""
    echo "Environment Variables:"
    echo "  GIT_SERVER_URL        Git server URL (default: http://localhost:8080)"
}

# List available tests
list_tests() {
    echo "Available tests:"
    for test_file in "$TEST_DIR"/*.sh; do
        if [[ -f "$test_file" && "$test_file" != *"run_tests.sh" ]]; then
            test_name=$(basename "$test_file" .sh)
            echo "  - $test_name"
        fi
    done
}

# Main execution
main() {
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -l|--list)
            list_tests
            exit 0
            ;;
        -c|--clean)
            rm -rf "$TEMP_DIR"
            echo "Cleaned up $TEMP_DIR"
            exit 0
            ;;
        "")
            setup
            run_all_tests
            show_summary
            ;;
        *)
            setup
            run_test "$1"
            show_summary
            ;;
    esac
}

# Trap cleanup on exit
trap cleanup EXIT

# Run main function
main "$@"
