# Git Server Test Suite 🧪

This directory contains comprehensive tests for the WebAssembly Git server implementation.

## 🚀 Quick Start

```bash
# Make scripts executable
chmod +x *.sh

# Run all tests
./run_tests.sh

# Run a specific test
./run_tests.sh 01_health_check

# List available tests
./run_tests.sh --list

# Get help
./run_tests.sh --help
```

## 📋 Test Categories

### **Core Protocol Tests**
1. **01_health_check** - Basic server health and debug endpoints
2. **02_capability_advertisement** - Git Protocol v2 capability advertisement
3. **03_ls_refs** - ls-refs command testing with various options
4. **04_fetch_command** - fetch command testing (empty repo scenarios)
5. **05_object_info** - object-info command testing

### **Robustness Tests**
6. **06_error_handling** - Error conditions and malformed requests
7. **07_git_client_compatibility** - Real Git client integration
8. **08_packet_line_validation** - Git packet-line protocol validation
9. **09_state_persistence** - Repository state consistency and persistence

## 🔧 Configuration

Set environment variables to customize test behavior:

```bash
# Change server URL (default: http://localhost:8080)
export GIT_SERVER_URL="http://localhost:3000"

# Run tests
./run_tests.sh
```

## 📊 Test Output

Tests provide:
- ✅ **Pass/Fail status** for each test case
- 🔍 **Detailed error messages** when tests fail
- 📋 **Sample responses** and protocol traces
- 📈 **Summary statistics** at the end

## 🧩 Individual Test Details

### **Health Check (01)**
- Tests basic server endpoints (`/`, `/refs`, `/objects`)
- Validates JSON response format
- Ensures server is responsive

### **Capability Advertisement (02)**
- Tests Git smart HTTP info/refs endpoint
- Validates Protocol v2 advertisement
- Checks for required capabilities (ls-refs, fetch, object-info)
- Validates packet-line format

### **ls-refs Command (03)**
- Tests basic reference listing
- Tests with symrefs, peel, unborn options
- Tests ref-prefix filtering
- Validates response format for empty repositories

### **fetch Command (04)**
- Tests basic fetch operations
- Tests with various capabilities (no-progress, thin-pack, ofs-delta, sideband-all)
- Handles empty repository scenarios
- Tests invalid want object handling

### **object-info Command (05)**
- Tests object metadata queries
- Tests with size, content attributes
- Tests content-limit and content-encoding options

### **Error Handling (06)**
- Tests invalid command rejection
- Tests malformed packet-line handling
- Tests unsupported services
- Tests invalid HTTP methods
- Tests large request body handling

### **Git Client Compatibility (07)**
- Tests with real `git` commands
- Tests `git ls-remote` with Protocol v2
- Tests `git clone` (expects appropriate handling of empty repo)
- Tests custom user agents and HTTP configurations
- Captures protocol traces for debugging

### **Packet-line Validation (08)**
- Validates Git packet-line protocol compliance
- Tests packet-line header format (4-digit hex)
- Tests flush packet handling
- Parses and validates multiple packet-lines
- Tests UTF-8 content encoding

### **State Persistence (09)**
- Tests repository state consistency
- Tests concurrent request handling
- Tests state stability after protocol operations
- Validates Theater actor memory consistency

## 🐛 Debugging Failed Tests

When tests fail:

1. **Check the server logs** - Look at Theater/git-server output
2. **Check temp files** - Test responses are saved in `/tmp/git-server-tests/`
3. **Run individual tests** - Use `./run_tests.sh <test_name>` for focused debugging
4. **Check Protocol traces** - Git client tests include `GIT_TRACE_PACKET` output

## 📝 Common Test Scenarios

### **Empty Repository Testing**
Most tests are designed to work with empty repositories:
- ls-refs should return empty ref list with flush packet
- fetch should handle "done" without objects gracefully
- Git clients may show warnings but shouldn't crash

### **Protocol v2 Compliance**
Tests validate:
- Proper capability advertisement format
- Packet-line protocol compliance
- Command routing and response structure
- Error handling for unsupported features

### **Concurrency and State**
Tests verify:
- Multiple simultaneous requests work correctly
- Repository state remains consistent
- Actor memory management works properly

## 🚦 Exit Codes

- **0** - All tests passed
- **1** - Some tests failed (check individual test output)

## 📚 Adding New Tests

To add a new test:

1. Create `tests/XX_test_name.sh`
2. Follow the existing pattern:
   ```bash
   #!/bin/bash
   SERVER_URL="$1"
   TEMP_DIR="$2"
   # Test implementation
   ```
3. Add the test to the `run_all_tests()` function in `run_tests.sh`
4. Make the script executable: `chmod +x XX_test_name.sh`

## 🎯 Best Practices

- **Test empty repositories first** - Your server starts empty
- **Add test data gradually** - Build up repository state for advanced tests  
- **Test error conditions** - Ensure graceful failure handling
- **Validate protocol compliance** - Git is strict about packet-line format
- **Test real Git clients** - Ultimate compatibility test

---

**Happy Testing!** 🎉

These tests help ensure your WebAssembly Git server correctly implements Git Protocol v2 and integrates well with the Theater actor system.
