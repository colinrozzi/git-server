# Git Server Test Suite

A focused, minimal test suite following the **Signal, Not Noise** principle.

## Quick Start

```bash
# Run all essential tests
./run_tests.sh

# Run specific test
./run_tests.sh ls_refs
./run_tests.sh git_push
```

## Test Philosophy

- **Success = Silent** - Just `✓ test_name`
- **Failure = Full Context** - Everything needed to debug
- **Essential tests only** - Focus on core functionality

## Current Tests

- **ls_refs** - Protocol v2 ls-refs command functionality
- **git_push** - Real git client push to empty repository

## Output Examples

**Clean success:**
```
Running essential git-server tests...

✓ ls_refs
✓ git_push

Summary: 2/2 tests passed
```

**Helpful failure:**
```
✗ ls_refs_response_format
  Expected: Response ending with flush packet (0000)
  Actual: 001b -> "invalid packet format"
  Parsed packet-line data:
    000e -> "001b001700"
    001b -> [packet too short]
  Issue: ls-refs response not properly terminated
  Check: handle_ls_refs_command() in protocol/http.rs
```

## Adding Tests

Follow the pattern in `ls_refs.sh` and `git_push.sh`:

1. Use helper functions from `test_helpers.sh`
2. Exit 0 on success (no output needed)
3. Use `test_fail()` with full context on failure
4. Focus on essential functionality only

## Old Tests

Previous verbose tests moved to `old_tests/` directory for reference.
