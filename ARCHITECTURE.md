# Git Server Actor Architecture

## Overview

This document explains the architecture of the Git Server Actor, which implements the Git Dumb HTTP Protocol as a WebAssembly component running in the Theater actor system.

## System Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Git Client                               │
│                    (git clone, push, etc.)                     │
└─────────────────────────┬───────────────────────────────────────┘
                          │ Git Dumb HTTP Protocol
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Theater Runtime                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │               Git Server Actor                            │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │            HTTP Framework                           │  │  │
│  │  │  • Route registration (/info/refs, /objects/*, etc) │  │  │
│  │  │  • Request routing and parsing                      │  │  │
│  │  │  • Response handling with proper headers           │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │        Git Dumb HTTP Protocol Handler               │  │  │
│  │  │  • Repository discovery (/info/refs, /HEAD)        │  │  │
│  │  │  • Object serving (/objects/xx/xxxxxxx)            │  │  │
│  │  │  • Reference serving (/refs/heads/branch)          │  │  │
│  │  │  • Push operations (PUT endpoints)                 │  │  │
│  │  │  • WebDAV locking (LOCK/DELETE)                    │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │           Repository State                          │  │  │
│  │  │  • In-memory Git objects (blob/tree/commit/tag)    │  │  │
│  │  │  • References (branches/tags) mapping              │  │  │
│  │  │  • SHA-1 hash validation                           │  │  │
│  │  │  • State persistence across requests               │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. Git Server Actor (WebAssembly Component)

**Location**: `src/lib.rs`
**Purpose**: Main actor implementation that coordinates all Git server functionality

**Key Responsibilities**:
- Initialize HTTP server and register Git protocol routes
- Handle incoming HTTP requests and route to appropriate handlers
- Maintain repository state between requests
- Integrate with Theater actor lifecycle

**Important Functions**:
- `init()` - Sets up HTTP server and routes, initializes repository
- `handle_request()` - Routes Git protocol requests to handlers
- State serialization/deserialization for persistence

### 2. Git Dumb HTTP Protocol Handler

**Location**: `src/protocol/dumb_http.rs`
**Purpose**: Implements the Git Dumb HTTP Protocol for repository operations

**Key Responsibilities**:
- Handle repository discovery requests (`/info/refs`, `/HEAD`)
- Serve individual Git objects (`/objects/xx/xxxxxxx`)
- Serve reference files (`/refs/heads/branch`)
- Handle push operations (PUT requests for objects and refs)
- Support WebDAV locking for concurrent access

**Protocol Endpoints**:

#### Read Operations (Clone/Fetch)
- `GET /info/refs` → List all repository references
- `GET /HEAD` → Get current HEAD reference
- `GET /objects/{hash[0:2]}/{hash[2:]}` → Retrieve Git object
- `GET /refs/heads/{branch}` → Get branch reference
- `GET /refs/tags/{tag}` → Get tag reference

#### Write Operations (Push)
- `PUT /objects/{hash[0:2]}/{hash[2:]}` → Upload Git object
- `PUT /refs/heads/{branch}` → Update branch reference
- `LOCK /refs/heads/{branch}` → Lock reference for update
- `DELETE /refs/heads/{branch}` → Release reference lock

#### Debug Operations
- `GET /` → Repository information
- `GET /debug/refs` → List all references
- `GET /debug/objects` → List all objects

### 3. Repository State Management

**Location**: `src/git/repository.rs`
**Purpose**: Manages in-memory Git repository state and object storage

**Data Structures**:
```rust
struct GitRepoState {
    repo_name: String,                    // Repository name
    refs: HashMap<String, String>,       // ref-name → commit-hash
    objects: HashMap<String, GitObject>, // object-hash → object-data
    head: String,                        // HEAD reference (e.g., "refs/heads/main")
}

enum GitObject {
    Blob { content: Vec<u8> },
    Tree { entries: Vec<TreeEntry> },
    Commit { tree: String, parents: Vec<String>, author: String, committer: String, message: String },
    Tag { object: String, tag_type: String, tagger: String, message: String },
}
```

**Key Operations**:
- `add_object()` - Store Git object with hash validation
- `update_ref()` - Update branch/tag references
- `serialize_tree_object()` - Convert tree to Git format
- `serialize_commit_object()` - Convert commit to Git format
- State validation and integrity checking

### 4. Git Object Management

**Location**: `src/git/objects.rs`
**Purpose**: Define Git object types and their operations

**Object Types**:
- **Blob**: File content storage
- **Tree**: Directory structure with file/subdirectory entries
- **Commit**: Snapshot with tree reference, parents, and metadata
- **Tag**: Named reference to any Git object

**Object Format**:
All objects follow Git's standard format:
```
<type> <size>\0<content>
```

Where content varies by type:
- Blob: Raw file bytes
- Tree: Mode/name/hash entries
- Commit: tree/parent/author/committer/message lines
- Tag: object/type/tagger/message lines

### 5. Hash and Compression Utilities

**Location**: `src/utils/hash.rs`, `src/utils/compression.rs`
**Purpose**: SHA-1 hash calculation and zlib compression for Git compatibility

**Hash Operations**:
- `calculate_git_hash_raw()` - Calculate SHA-1 with Git object header
- `calculate_git_hash()` - Calculate hash for GitObject instances
- Hash validation for object integrity

**Compression Operations**:
- `compress_zlib()` - Compress data using zlib (Git loose object format)
- `decompress_zlib()` - Decompress zlib data for object parsing
- Proper Adler-32 checksum handling

## Protocol Flow

### Clone Operation
```
1. git clone http://localhost:8080 repo
2. GET /info/refs → Returns all available references
3. GET /HEAD → Returns current HEAD reference  
4. GET /refs/heads/main → Returns main branch commit hash
5. GET /objects/xx/xxxx → Downloads commit object
6. GET /objects/xx/xxxx → Downloads tree object
7. GET /objects/xx/xxxx → Downloads blob objects
8. Git reconstructs working directory
```

### Push Operation
```
1. git push http://localhost:8080 main
2. LOCK /refs/heads/main → Acquire exclusive lock
3. PUT /objects/xx/xxxx → Upload new blob objects
4. PUT /objects/xx/xxxx → Upload new tree object
5. PUT /objects/xx/xxxx → Upload new commit object
6. PUT /refs/heads/main → Update branch reference
7. DELETE /refs/heads/main → Release lock
```

## State Persistence

The actor maintains repository state across requests using Theater's state persistence:

1. **Initialization**: State loaded from previous session or created fresh
2. **Request Processing**: State modified during Git operations
3. **Response**: Updated state serialized and saved
4. **Actor Restart**: State automatically restored from last save

## Error Handling

The implementation includes comprehensive error handling:

- **HTTP Level**: Proper status codes (200, 404, 400, etc.)
- **Git Level**: Object validation, hash verification, reference checking
- **Actor Level**: State serialization errors, HTTP server failures
- **Logging**: Detailed logging for debugging and monitoring

## Security Considerations

Current implementation focuses on functionality over security:

- **No Authentication**: All operations are anonymous
- **No Authorization**: No access control on repositories
- **Input Validation**: SHA-1 hash format and object structure validation
- **Memory Safety**: Rust's memory safety prevents buffer overflows

Future security enhancements could include:
- Token-based authentication
- Repository-level access control
- Rate limiting
- Audit logging

## Performance Characteristics

**Advantages of Dumb HTTP**:
- Simple request/response pattern
- Stateless operations
- HTTP caching friendly
- Easy to debug and monitor

**Trade-offs**:
- More HTTP requests than Smart HTTP
- No delta compression (larger transfer sizes)
- Higher latency for large repositories

**Scalability**:
- Memory usage proportional to repository size
- CPU usage dominated by SHA-1 calculations and zlib compression
- Network usage higher than Smart HTTP but cacheable

## Future Enhancements

### Short Term
- Multi-repository support (different repository names)
- Basic authentication and authorization
- HTTP caching headers for better performance
- Comprehensive error messages

### Medium Term
- Pack file generation for efficient large repository handling
- Repository management API (create/delete repositories)
- Web interface for repository browsing
- Metrics and monitoring endpoints

### Long Term
- Repository replication and synchronization
- Integration with external authentication systems
- Advanced caching strategies
- Support for Git LFS (Large File Storage)

## Development Guidelines

### Adding New Features
1. Update the appropriate module (`protocol/`, `git/`, `utils/`)
2. Add comprehensive logging for debugging
3. Update tests and documentation
4. Ensure proper error handling
5. Validate Git protocol compliance

### Testing Strategy
- Unit tests for individual components
- Integration tests with real Git clients
- Performance testing with various repository sizes
- Error condition testing (network failures, invalid data)

### Code Organization
- Keep protocol logic separate from object management
- Use proper error types instead of strings
- Follow Rust best practices for WebAssembly
- Maintain compatibility with Theater actor model

This architecture provides a solid foundation for a Git server that's simple to understand, maintain, and extend while remaining fully compatible with standard Git clients.
