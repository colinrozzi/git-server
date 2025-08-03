# Ref Management for Push Operations
**Status**: 📋 TODO  
**Priority**: Medium  
**Dependencies**: repository-updates

## Goal
Handle the creation and management of Git refs during push operations, establishing the proper branch structure after pack file processing.

## Ref Understanding in Push Context
In Git push operations, refs represent the branch/tag structure and map names to specific object hashes.

## Creation Patterns for Empty Repository
1. **Basic Flow**: First commit creates `refs/heads/main` (or master)
2. **Branch Mapping**: Optional branch name specification
3. **Default Branch**: Handle when client pushes `HEAD` ref separately

## Ref Types to Support
- **Branch refs**: `refs/heads/main`, `refs/heads/master`, `refs/heads/<branch>`
- **Tag refs**: `refs/tags/v1.0.0` (future, but keep in mind)
- **HEAD**: Special ref pointing to current branch

## Ref Operations
- **Create**: Add new ref with object hash
- **Update**: Modify existing ref hash (for updates, not creates)
- **Validate**: Ensure refs point to valid objects in repository
- **Sanitize**: Handle branch names and ensure proper format

## Branch Creation Rules
For empty repository pushes:
1. **First commit** → Creates `refs/heads/main` (or `master`)
2. **Auto HEAD**: If ref starts with `refs/heads/`, set HEAD to that ref
3. **Preserve ordering**: Allow any valid branch name the client specifies

## Integration with Repository Updates
- **Ref creation** happens after pack file parsing and object storage
- **Object validation** ensures all ref targets exist in objects map
- **HEAD management** automatically updates to point to new branch
- **Ref structure** follows Git conventions

## Testing Specific Cases
1. **First commit**: Creates `refs/heads/main` and sets HEAD
2. **Custom branch**: Creates `refs/heads/develop` correctly
3. **Multiple branches**: Handle future multi-branch pushes
4. **Root commit**: Single commit ref points to commit object

## Error Handling
- **Invalid ref names** → Reject with clear error
- **Missing objects** → Ensure refs point to existing commits
- **Format issues** → Follow Git ref naming conventions
- **Existing conflicts** → For empty repo, only create operations allowed

## Status Dependencies
- **Success indication**: Ref should exist in response
- **Validation**: Ensure ref creation matches client expectations
- **Persistence**: State saved with new refs in GitRepoState

## Minimal Implementation
For empty repository specifically:
- Only handle ref creation (not updates/deletes)
- Focus on `refs/heads/` prefixes
- Auto-update HEAD to point to first branch created