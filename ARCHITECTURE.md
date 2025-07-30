# Git Server Architecture

## Overview

This document explains the architecture of the Git Server Actor, helping new contributors understand how the components work together to create a WebAssembly-based git remote server.

## System Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Git Client                               │
│                    (git clone, push, etc.)                     │
└─────────────────────────┬───────────────────────────────────────┘
                          │ HTTP/Git Protocol
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Theater Runtime                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │               Git Server Actor                            │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │            HTTP Framework                           │  │  │
│  │  │  • Route registration                               │  │  │
│  │  │  • Request routing                                  │  │  │
│  │  │  • Response handling                                │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │          Git Protocol Handler                       │  │  │
│  │  │  • Smart HTTP Transport                            │  │  │
│  │  │  • Packet-line encoding                            │  │  │
│  │  │  • Pack negotiation                                │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │           Repository State                          │  │  │
│  │  │  • In-memory git objects                           │  │  │
│  │  │  • References (branches/tags)                      │  │  │
│  │  │  • State persistence                               │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. Git Server Actor (WebAssembly Component)

**Location**: `src/lib.rs`
**Purpose**: Main actor implementation that coordinates all git server functionality

**Key Responsibilities**:
- Initialize HTTP server and routes during actor startup
- Maintain git repository state in memory
- Handle HTTP requests from git clients
- Implement git protocol specifications
- Manage actor lifecycle and error handling

**Interfaces Implemented**:
- `theater:simple/actor` - Basic actor lifecycle
- `theater:simple/http-handlers` - HTTP request handling

### 2. HTTP Framework Integration

**Purpose**: Bridges Theater's HTTP capabilities with git protocol requirements

**Setup Process** (in `init()` function):
```rust
// 1. Create HTTP server
let server_id = http_framework::create_server(&config)?;

// 2. Register handler for git operations  
let git_handler = http_framework::register_handler("git")?;

// 3. Map git protocol routes to handler
let routes = [
    ("/info/refs", "GET", git_handler),      // Discovery
    ("/git-upload-pack", "POST", git_handler), // Clone/fetch
    ("/git-receive-pack", "POST", git_handler), // Push
];

// 4. Start server
http_framework::start_server(server_id)?;
```

**Request Flow**:
1. Git client makes HTTP request
2. Theater HTTP framework receives request
3. Framework routes to registered git handler
4. Actor's `handle_request()` method called
5. Request routed to appropriate git protocol handler
6. Response sent back through framework

### 3. Git Protocol Implementation

**Purpose**: Implements Git Smart HTTP Transport Protocol specification

#### Discovery Phase (`handle_info_refs`)
- Handles `GET /info/refs?service=git-upload-pack`
- Returns repository capabilities and references
- Uses packet-line protocol format
- **Status**: ✅ Complete - works with real git clients

#### Data Transfer Phase (`handle_upload_pack`, `handle_receive_pack`)  
- Handles `POST /git-upload-pack` (clone/fetch)
- Handles `POST /git-receive-pack` (push)
- Manages want/have negotiation
- Generates/parses pack files
- **Status**: 🚧 In progress - discovery works, pack negotiation needed

#### Packet-Line Protocol
Git uses a specific wire format where each line is prefixed with its length:
```rust
fn format_pkt_line(line: &str) -> Vec<u8> {
    let len = line.len() + 4;
    let len_hex = format!("{:04x}", len);
    let mut result = len_hex.into_bytes();
    result.extend(line.as_bytes());
    result
}
```

### 4. Repository State Management

**Purpose**: Maintains git repository data in actor memory

**State Structure**:
```rust
struct GitRepoState {
    repo_name: String,                    // Repository identifier
    refs: HashMap<String, String>,        // branch/tag -> commit hash  
    objects: HashMap<String, GitObject>,  // object hash -> object data
    head: String,                         // Current HEAD reference
}
```

**Object Types**:
- **Blob**: File contents
- **Tree**: Directory listings  
- **Commit**: Commit metadata + tree reference
- **Tag**: Annotated tag information

**Persistence**: 
- State serialized/deserialized as JSON between requests
- Theater handles persistence and recovery
- No external database required

## Data Flow

### Git Clone Request Flow

```
1. Git Client: git clone http://localhost:8080
   │
   ▼
2. Discovery Request: GET /info/refs?service=git-upload-pack
   │
   ▼  
3. Theater HTTP Framework routes to Git Server Actor
   │
   ▼
4. Actor.handle_request() -> handle_info_refs()
   │
   ▼
5. Generate packet-line response with repository refs
   │
   ▼
6. Git Client receives refs, starts pack negotiation
   │
   ▼
7. Pack Request: POST /git-upload-pack (want/have data)
   │
   ▼
8. Actor.handle_request() -> handle_upload_pack()
   │
   ▼
9. 🚧 Parse wants/haves, generate pack file, send ACK/NAK
   │
   ▼
10. Git Client receives pack data, creates local repository
```

### Current Implementation Status

**✅ Working (Steps 1-6)**:
- Git client can discover repository
- Packet-line protocol correctly implemented
- HTTP routing functional
- State management working

**🚧 In Progress (Steps 7-10)**:
- Pack negotiation parsing needed
- Pack file generation required
- ACK/NAK response implementation

## Extension Points

### Adding New Git Operations

To add support for new git operations:

1. **Add route** in `init()`:
```rust
("/new-endpoint", "GET", git_handler)
```

2. **Add handler** in `handle_request()`:  
```rust
("GET", "/new-endpoint") => handle_new_operation(&repo_state, &request)
```

3. **Implement handler function**:
```rust
fn handle_new_operation(repo_state: &GitRepoState, request: &HttpRequest) -> HttpResponse {
    // Implementation here
}
```

### Adding Repository Features

To extend repository functionality:

1. **Update State Structure**:
```rust
struct GitRepoState {
    // ... existing fields
    new_feature: HashMap<String, FeatureData>,
}
```

2. **Add State Migration**:
```rust
fn migrate_state(old_state: OldGitRepoState) -> GitRepoState {
    // Handle backward compatibility
}
```

3. **Update Serialization**:
```rust
let new_state = serde_json::to_vec(&updated_repo_state)?;
```

### Theater Actor Integration

To add new Theater capabilities:

1. **Add Handler in Manifest**:
```toml
[[handler]]
type = "new-handler"
config = { /* options */ }
```

2. **Import in Code**:
```rust
use bindings::theater::simple::new_handler;
```

3. **Use in Actor**:
```rust
let result = new_handler::some_operation(data)?;
```

## Performance Considerations

### Memory Usage
- Repository state kept entirely in memory
- Large repositories may require optimization
- Consider implementing object streaming for large repos

### Request Handling
- Single-threaded actor model (Theater handles concurrency)
- State mutations are atomic within single request
- Long-running operations should yield control

### Pack File Generation
- Most computationally expensive operation
- Consider caching pack files for popular refs
- Implement incremental pack generation

## Security Model

### WebAssembly Sandboxing
- Actor runs in isolated WASM environment
- Can only access explicitly granted capabilities
- Theater enforces resource limits

### Git Protocol Security
- Currently no authentication implemented
- All operations are public
- Future: Add authentication layer

### Theater Security
- All operations logged in event chain
- Audit trail for all git operations
- Supervision tree provides fault isolation

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_packet_line_format() {
        let result = format_pkt_line("hello");
        assert_eq!(result, b"0009hello");
    }
}
```

### Integration Tests
```bash
# Test with real git client
git ls-remote http://localhost:8080
git clone http://localhost:8080 test-repo
```

### Protocol Compliance Tests
```bash
# Test discovery phase
curl "http://localhost:8080/info/refs?service=git-upload-pack"

# Test pack negotiation (future)
curl -X POST http://localhost:8080/git-upload-pack -d "want ..."
```

## Debugging Guide

### Common Issues

**"Actor failed to start"**
- Check component compilation: `cargo component build --release`
- Verify manifest.toml points to correct WASM file
- Check Theater server logs: `theater-server --log-stdout`

**"Connection refused"**
- Verify HTTP server started successfully
- Check port 8080 is available
- Look for HTTP framework setup errors in logs

**"Git protocol errors"**
- Check packet-line formatting
- Verify Content-Type headers
- Compare responses with git protocol spec

### Debugging Tools

**Theater CLI**:
```bash
theater list              # Show running actors
theater events <actor-id> # View actor event chain
theater stop <actor-id>   # Stop specific actor
```

**HTTP Debugging**:
```bash
curl -v http://localhost:8080/           # Verbose HTTP request
wireshark                                # Capture network traffic
git -c http.verbose=true clone ...       # Verbose git client
```

**Component Debugging**:
```bash
wasm-objdump -x target/wasm32-wasip1/release/git_server.wasm  # Inspect WASM
wasm-validate target/wasm32-wasip1/release/git_server.wasm    # Validate WASM
```

## Future Architecture Considerations

### Scaling
- **Multiple Repositories**: One actor per repository vs shared actors
- **Load Balancing**: Distribute repositories across multiple actors
- **Caching**: Implement pack file and object caching strategies

### Persistence
- **Database Integration**: Store objects in external database
- **File System**: Use Theater filesystem handler for object storage
- **Distributed Storage**: Replicate across multiple Theater nodes

### Microservices Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Git Frontend  │    │  Auth Service   │    │ Object Storage  │
│     Actor       │◄──►│     Actor       │    │     Actor       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                                              ▲
         ▼                                              │
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Pack Generator │    │   Ref Manager   │    │   Event Logger  │
│     Actor       │    │     Actor       │    │     Actor       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Integration Opportunities
- **CI/CD Pipelines**: Trigger builds on push
- **Code Review**: Integration with review systems  
- **Backup Systems**: Automated repository backup
- **Monitoring**: Repository usage and performance metrics

## Contributing Guidelines

### Code Style
- Follow Rust standard formatting: `cargo fmt`
- Use clippy for linting: `cargo clippy`
- Document public APIs with `///` comments
- Add unit tests for new functionality

### Git Protocol Changes
- Refer to official Git documentation
- Test with multiple git client versions
- Ensure backward compatibility
- Add protocol compliance tests

### Theater Integration
- Follow Theater actor patterns
- Use proper error handling
- Log important events
- Respect resource limits

### Documentation
- Update README.md for user-facing changes
- Update this architecture doc for internal changes
- Add inline code comments for complex logic
- Include examples in documentation

---

**This architecture enables a scalable, secure, and observable git server built on modern WebAssembly and actor system foundations.**
