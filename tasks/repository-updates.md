# Repository State Updates
**Status**: 📋 TODO  
**Priority**: Medium  
**Dependencies**: pack-file-parsing

## Goal
Handle the storage of new Git objects and updating the repository state after a successful receive-pack operation.

## Current State
- **GitRepoState** tuple contains: objects, refs, head, repo_name
- **Serialization**: Uses serde_json for state persistence
- **Objects**: Stored as `HashMap<String, GitObject>`
- **Refs**: Stored as `HashMap<String, String>` mapping ref names to object IDs

## Update Operations Required
1. **Object Integration**: Add parsed GitObject instances to objects map
2. **Ref Creation**: Add new refs with their corresponding object hashes
3. **HEAD Update**: Ensure HEAD points to correct ref after push
4. **State Persistence**: Ensure updates are saved to the Theater actor state

## Update Flow
```
After successful packet parse:
├── For each object in pack:
│   ├── Validate uniqueness (skip duplicates)
│   ├── Insert into GitRepoState.objects
│   └── Ensure hash integrity
├── For each received ref update:
│   ├── If create: Add new ref to GitRepoState.refs
│   ├── If update: Update existing ref hash
│   └── If delete: Remove ref from GitRepoState.refs
└── Update HEAD if main/master branch created
```

## State Consistency Rules
- **Atomicity**: All updates succeed or none (simple for our single instance)
- **Integrity**: Object references must exist in objects map
- **Naming**: Follow Git ref conventions (refs/heads/branch-name)
- **Validation**: Objects must be valid Git object format

## Integration Points
- **receive-pack-handler**: Calls this after parsing pack file
- **ref-management**: Uses updated state for ref operations
- **status-reporting**: Uses updated state to verify operation success

## Edge Cases
- **Duplicate objects**: Handle with graceful skip (not error)
- **Existing refs**: Prevent overwrite without explicit update (push -f would need handling)
- **HEAD conflicts**: Ensure HEAD points to valid ref
- **Empty pushes**: Handle edge case where pack contains no objects

## Testing Validation
- **Object count**: Verify total objects match pack contents
- **Ref existence**: Ensure all pushed refs appear in map
- **Object access**: Confirm all ref targets exist in objects
- **State persistence**: Actor state updates correctly between operations

## Performance Considerations
- **Memory**: Objects stored in-memory, suitable for demonstration use
- **State size**: Serialize entire state for persistence (limitation of approach)
- **Initialization**: Empty server starts clean, grows as objects added

## Dependencies This Enables
- ref-management (uses updated repository state)
- status-reporting (utilizes updated state to validate operations)