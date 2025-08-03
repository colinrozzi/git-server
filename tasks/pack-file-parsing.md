# Pack File Parsing Infrastructure
**Status**: 📋 TODO  
**Priority**: High  
**Dependencies**: --

## Goal
Create the ability to parse Git pack files received via the receive-pack endpoint.

## Current State
- `PackReader` exists for generating pack files (unused stub)
- No parsing of incoming pack files
- No understanding of pack file structure

## Pack File Structure Understanding
Based on Git documentation, pack files have:

1. **Header**: `PACK` + version (4 bytes)
2. **Object count**: 4 bytes little-endian
3. **Objects**: Deflated object data with size/type info
4. **Checksum**: 20-byte SHA-1 of entire pack

## Object Types to Support
- **Blob** - File content (lowest priority for basic flow)
- **Commit** - Commit objects with tree refs, parents, etc
- **Tree** - Directory structures with entries pointing to blobs/trees

## Parsing Requirements
- **Incremental**: Don't load entire pack into memory
- **Validation**: Verify checksums and object integrity
- **Storage**: Convert to internal GitObject format
- **Error handling**: Invalid pack file detection

## Implementation Plan
```
GitPackDecoder
├── parse_header()
├── parse_objects() → Vec<GitObject>
├── validate_checksum()
└── handle_object_refs() // connect objects to refs
```

## Integration Points
- **receive-pack-handler** - Uses this to parse incoming pack data
- **repository-updates** - Uses parsed objects to update state
- **ref-management** - Uses objects to create new refs

## Scope Questions (to resolve)
- **Minimal scope**: Only parse what we absolutely need for basic push?
- **Error recovery**: How to handle partial/corrupt packs?
- **Performance**: Keep simple implementations for now?

## Testing Strategy
1. **Create test packs** from real Git repositories
2. **Round-trip** test: parse pack → store objects → generate same pack
3. **Integration** with receive-pack flow

## Dependencies This Unblocks
- receive-pack-handler (needs to parse incoming packs)
- repository-updates (needs parsed objects)
- ref-management (needs objects to create refs for)