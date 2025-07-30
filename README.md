# Git Server Actor

🚀 **A WebAssembly git repository server built on the Theater actor system**

This project implements a Git Smart HTTP Transport Protocol server as a Theater actor, enabling git clients to clone, fetch, and push to repositories served by WebAssembly components in a distributed actor system.

## 🌟 What Makes This Special

- **WebAssembly Git Server** - Full git remote server running as a WASM component
- **Theater Actor Integration** - Leverages Theater's supervision, messaging, and event chain systems  
- **Real Git Protocol** - Implements Git Smart HTTP Transport Protocol (RFC)
- **Production Ready Architecture** - Built for scalability, security, and observability

## 🎯 Current Status

### ✅ **Working Features**
- **HTTP Framework Integration** - Proper server setup with routes ✅
- **Git Protocol Discovery** - `git ls-remote` works with real git clients ✅
- **Repository Information** - REST endpoints for debugging and inspection ✅
- **Packet-Line Protocol** - Correct git wire protocol implementation ✅
- **Want/Have Negotiation** - Parses client requests and responds with ACK/NAK ✅
- **Pack Protocol Foundation** - Generates and sends pack files to clients ✅
- **Repository Object Creation** - Auto-generates README.md, tree, and commit objects ✅
- **In-Memory State** - Repository refs and objects stored in actor state ✅

### 🚧 **Nearly Complete**
- **Git Clone Protocol** - Handles full clone negotiation, minor pack format issues remaining
- **Object Storage** - Real git commits, trees, and blobs (✅ basic implementation)
- **Pack File Generation** - Creates pack files with proper packet-line wrapping

### 🎯 **Demo Status**
```bash
# This works! ✅
git ls-remote http://localhost:8080
0d6c032588a90a8fa014618b8c784751000000b9	refs/heads/main

# This nearly works! 🚧 (gets to pack transfer)
git clone http://localhost:8080 test-repo
# Completes discovery ✅, negotiation ✅, pack transfer 🚧
```

## 🏗️ Architecture

### **Theater Actor Pattern**
```
┌─────────────────────────────────────────────────────┐
│                Git Server Actor                     │
├─────────────────────────────────────────────────────┤
│  • In-memory git repository state                   │
│  • HTTP framework integration                       │
│  • Git Smart HTTP protocol handlers                 │
│  • Theater supervision & messaging                  │
└─────────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────┐
│               Theater Runtime                       │
├─────────────────────────────────────────────────────┤
│  • WebAssembly component execution                  │
│  • HTTP server management                           │
│  • Actor supervision tree                           │
│  • Event chain & state persistence                  │
└─────────────────────────────────────────────────────┘
```

### **Git Protocol Flow**
```
Git Client                    Git Server Actor
    │                              │
    │ GET /info/refs?service=...    │
    ├─────────────────────────────▶│ 
    │                              │ ✅ Discovery
    │ ◀─────────────────────────────┤ (packet-line format)
    │                              │
    │ POST /git-upload-pack        │
    ├─────────────────────────────▶│ 
    │                              │ ✅ Want/Have Parsing
    │ ◀─────────────────────────────┤ ✅ ACK/NAK Response
    │                              │ 🚧 Pack File Transfer
```

### **State Structure**
```rust
struct GitRepoState {
    repo_name: String,
    refs: HashMap<String, String>,        // branch -> commit hash
    objects: HashMap<String, GitObject>,  // hash -> object data  
    head: String,                         // current HEAD ref
}

enum GitObject {
    Blob { content: Vec<u8> },
    Tree { entries: Vec<TreeEntry> },
    Commit { tree: String, parents: Vec<String>, /* ... */ },
    Tag { object: String, /* ... */ },
}
```

## 🚀 Quick Start

### **Prerequisites**
- [Theater CLI](https://github.com/colinrozzi/theater) installed
- Rust 1.81.0+ with `wasm32-wasip1` target
- `cargo-component` for WebAssembly builds

### **1. Build the Component**
```bash
cd git-server
cargo component build --release
```

### **2. Start the Git Server**
```bash
theater start manifest.toml
```

### **3. Test with Git**
```bash
# Test repository discovery
git ls-remote http://localhost:8080

# Try cloning (will partially work)
git clone http://localhost:8080 test-repo
```

### **4. Debug Endpoints**
```bash
# Repository info
curl http://localhost:8080/

# List branches and tags  
curl http://localhost:8080/refs

# List git objects
curl http://localhost:8080/objects

# Git protocol discovery
curl "http://localhost:8080/info/refs?service=git-upload-pack"
```

## 🛠️ Development Guide

### **Project Structure**
```
git-server/
├── src/
│   ├── lib.rs              # Main actor implementation
│   └── bindings.rs         # Generated WIT bindings
├── wit/                    # WebAssembly Interface Types
├── manifest.toml           # Theater actor configuration
├── Cargo.toml             # Rust project configuration
└── README.md              # This file
```

### **Key Components**

#### **Actor Initialization (`init`)**
- Creates HTTP server on port 8080
- Registers git protocol routes
- Initializes empty repository state
- Sets up Theater actor lifecycle

#### **HTTP Handler (`handle_request`)**
- Routes git protocol requests
- Implements Git Smart HTTP Transport
- Handles discovery and data transfer phases
- Maintains repository state across requests

#### **Git Protocol Handlers**
- `handle_info_refs` - Repository discovery (✅ Working)
- `handle_upload_pack` - Clone/fetch data with full negotiation (🚧 Nearly complete)
- `handle_receive_pack` - Push data (🚧 Planned)

#### **Pack Protocol Implementation**
- `parse_upload_pack_request` - Parses want/have lines from packet-line format ✅
- `ensure_minimal_repo_objects` - Creates real git objects (README + tree + commit) ✅
- `generate_pack_file` - Creates Git pack files with proper headers ✅
- `format_pack_data` - Wraps pack data in packet-line format ✅

### **Development Workflow**

#### **1. Make Changes**
```bash
# Edit source code
vim src/lib.rs

# Rebuild component
cargo component build --release
```

#### **2. Test Changes**
```bash
# Restart actor (Theater hot-reloads automatically)
theater stop <actor-id>
theater start manifest.toml

# Test with git client
git ls-remote http://localhost:8080
```

#### **3. Debug Issues**
```bash
# View actor logs
theater events <actor-id>

# Check actor status
theater list

# Test HTTP endpoints directly
curl -v http://localhost:8080/
```

## 📡 Git Protocol Implementation

### **Git Smart HTTP Transport Protocol**

This server implements the [Git Smart HTTP Transport Protocol](https://git-scm.com/docs/http-protocol), which consists of:

#### **Discovery Phase** ✅
```http
GET /info/refs?service=git-upload-pack HTTP/1.1

200 OK
Content-Type: application/x-git-upload-pack-advertisement

001e# service=git-upload-pack
0000
003d0000000000000000000000000000000000000000 refs/heads/main
0000
```

#### **Data Transfer Phase** 🚧  
```http
POST /git-upload-pack HTTP/1.1
Content-Type: application/x-git-upload-pack-request

0032want 0000000000000000000000000000000000000000
0000

200 OK  
Content-Type: application/x-git-upload-pack-result

[ACK/NAK negotiation + pack data]
```

### **Packet-Line Protocol**
Git uses a packet-line format where each line is prefixed with its length in hex:
```
001e# service=git-upload-pack\n
├─┘ │
│   └─ Actual content (30 bytes total)  
└─ Length in hex (0x001e = 30)
```

## 🧩 Theater Integration

### **Handlers Used**
```toml
[[handler]]
type = "runtime"          # Logging and actor lifecycle

[[handler]]  
type = "http-framework"   # HTTP server and routing
```

### **Actor Lifecycle**
1. **Init** - Theater loads WASM component, calls `init()`
2. **HTTP Setup** - Actor creates server, registers routes
3. **Request Handling** - HTTP requests routed to `handle_request()`
4. **State Persistence** - Repository state maintained across requests
5. **Supervision** - Theater monitors actor health and restarts if needed

### **Event Chain Integration**
Every git operation is logged in Theater's event chain:
- Repository discoveries
- Clone attempts  
- Push operations
- Error conditions

## 🎯 Next Implementation Steps

### **Phase 1: Complete Clone Support**
- [ ] Parse want/have negotiation in `handle_upload_pack`
- [ ] Generate proper ACK/NAK responses
- [ ] Create pack files with requested objects
- [ ] Add real commit/tree/blob objects to repository

### **Phase 2: Push Support**  
- [ ] Implement `handle_receive_pack` for git push
- [ ] Parse incoming pack files
- [ ] Update repository refs on successful push
- [ ] Add push authorization/validation

### **Phase 3: Repository Management**
- [ ] API endpoints for creating repositories
- [ ] Repository initialization with real commits
- [ ] Branch and tag management
- [ ] Repository metadata and configuration

### **Phase 4: Advanced Features**
- [ ] Authentication and authorization
- [ ] Web interface for browsing repositories  
- [ ] Webhook notifications to other actors
- [ ] Repository mirroring and synchronization

## 🤝 Contributing

### **Getting Started**
1. Clone the actor-registry repo
2. Study the Theater documentation  
3. Review the Git Smart HTTP protocol spec
4. Look at existing Theater actors for patterns

### **Development Environment**
```bash
# Install Theater CLI
cargo install theater-server-cli theater-cli

# Start Theater server
theater-server --log-stdout

# Install WebAssembly tools
rustup target add wasm32-wasip1
cargo install cargo-component
```

### **Contribution Areas**
- 🐛 **Pack Protocol** - Implement want/have negotiation
- 🎨 **Web Interface** - Repository browsing UI
- 🔒 **Security** - Authentication and authorization  
- 📊 **Monitoring** - Metrics and observability
- 🧪 **Testing** - Integration tests with real git clients

## 📚 Resources

### **Git Protocol Documentation**
- [Git Smart HTTP Transport](https://git-scm.com/docs/http-protocol)
- [Git Pack Protocol](https://git-scm.com/docs/pack-protocol)  
- [Git Packet-Line Format](https://git-scm.com/docs/protocol-common)

### **Theater System**
- [Theater Documentation](https://colinrozzi.github.io/theater)
- [Actor Registry](https://github.com/colinrozzi/actor-registry) 
- [WebAssembly Components](https://component-model.bytecodealliance.org/)

### **Implementation References**
- [Git Source Code](https://github.com/git/git) - Reference implementation
- [libgit2](https://libgit2.org/) - Git implementation library
- [go-git](https://github.com/go-git/go-git) - Go git implementation

## 🏆 Acknowledgments

This project demonstrates the power of the Theater actor system for building distributed, observable, and secure infrastructure. The combination of WebAssembly's sandboxing, Theater's supervision, and Git's proven protocol creates a unique foundation for next-generation version control systems.

Special thanks to the WebAssembly Component Model and Theater communities for building the foundational technologies that make this possible.

---

**Built with ❤️ using Theater, WebAssembly, and Rust**
