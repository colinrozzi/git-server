# Receive-pack Capability Advertisement
**Status**: 📋 TODO  
**Priority**: High  
**Dependencies**: --

## Goal
Add receive-pack specific capabilities to the info/refs response for `service=git-receive-pack` requests.

## Current State
- Protocol v2 upload-pack capabilities work (`ls-refs`, `fetch`, `object-info`)
- No receive-pack capabilities are advertised

## Required Capabilities
Based on Protocol v2 docs, the receive-pack service should advertise:

```
0012version 2
0049agent=git-server/0.1.0
0021object-format=sha1
004areceive-pack=report-status delete-refs side-band-64k quiet atomic
000aofs-delta
0000
```

## Specific Capabilities Needed
- **receive-pack** - The main command with these features:
  - report-status - Send status back after push
  - delete-refs - Allow ref deletion
  - side-band-64k - Sideband for progress/status
  - quiet - Reduce progress output
  - atomic - All-or-nothing ref updates
  - ofs-delta - Optimize pack transmission

## Implementation Notes
- Add to existing `/info/refs` handler in `src/protocol/http.rs`
- Route based on `service=git-receive-pack` parameter
- Keep existing upload-pack advertisement separate
- Ensure proper Content-Type headers

## Scope Boundaries
- Just the advertisement - no actual implementation yet
- Maintain existing upload-pack functionality
- Don't implement the actual commands listed in receive-pack

## Dependencies on This Task
- receive-pack-handler - Will need these capabilities advertised

## Testing
1. `curl "http://localhost:8080/info/refs?service=git-receive-pack"` should show receive-pack capabilities
2. Explicitly should NOT break upload-pack capabilities