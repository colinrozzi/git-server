# Git Server Actor

🚀 **A WebAssembly git repository server built on the Theater actor system**

This project implements a Git Smart HTTP Transport Protocol server as a Theater actor, enabling git clients to clone, fetch, and push to repositories served by WebAssembly components in a distributed actor system.

## 🌟 What Makes This Special

- **WebAssembly Git Server** - Full git remote server running as a WASM component
- **Theater Actor Integration** - Leverages Theater's supervision, messaging, and event chain systems  
- **Real Git Protocol** - Implements Git Smart HTTP Transport Protocol (RFC)
- **Production Ready Architecture** - Built for scalability, security, and observability
- **Real SHA-1 Implementation** - Proper Git object hashing and pack file checksums

## 🏗️ Architecture

### **Theater Actor Pattern**
```
┌─────────────────────────────────────────────────────┐
│                Git Server Actor                     │
├─────────────────────────────────────────────────────┤
│  • In-memory git repository state                   │
│  • HTTP framework integration                       │
│  • Git Smart HTTP protocol handlers                 │
│  • Real SHA-1 object hashing                        │
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
    │ ◀─────────────────────────────┤ (real commit hashes)
    │                              │
    │ POST /git-upload-pack        │
    ├─────────────────────────────▶│ 
    │                              │ ✅ Want/Have Parsing
    │ ◀─────────────────────────────┤ ✅ ACK/NAK Response
    │                              │ ✅ Pack File Transfer
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
# Test repository discovery (works perfectly)
git ls-remote http://localhost:8080

# Try cloning (99% working - objects unpack successfully)
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
│   ├── lib.rs              # Main actor implementation with SHA-1
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
- Initializes repository with real Git objects
- Creates proper commit/tree/blob objects with SHA-1 hashes
- Sets up Theater actor lifecycle

#### **HTTP Handler (`handle_request`)**
- Routes git protocol requests
- Implements Git Smart HTTP Transport
- Handles discovery and data transfer phases
- Maintains repository state across requests

#### **Git Protocol Handlers**
- `handle_info_refs` - Repository discovery (✅ Working perfectly)
- `handle_upload_pack` - Clone/fetch data with full negotiation (🚧 99% complete)
- `handle_receive_pack` - Push data (🚧 Planned)

#### **Pack Protocol Implementation**
- `parse_upload_pack_request` - Parses want/have lines from packet-line format ✅
- `ensure_minimal_repo_objects` - Creates real git objects with proper SHA-1 ✅
- `generate_pack_file` - Creates Git pack files with dependency resolution ✅
- `add_object_with_dependencies` - Recursively includes required objects ✅
- `sha1_hash` - Real SHA-1 implementation for Git compatibility ✅

