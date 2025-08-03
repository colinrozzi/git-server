# Git Server Actor - Hybrid Protocol Implementation 🚀

**A pragmatic WebAssembly Git server with Protocol v2 for fetch operations and v1 compatibility for push operations**

This project implements a **hybrid approach** to Git protocol support: Protocol v2 for modern fetch operations where it's stable and mature, with Protocol v1 fallback for push operations where v2 support is still evolving across Git clients and hosting platforms.

## 🔄 Hybrid Protocol Strategy

### **Why Hybrid?**
- ✅ **Protocol v2 for fetch** - Mature, widely supported, significant performance benefits
- 🔄 **Protocol v1 for push** - Maximum compatibility while v2 push support stabilizes
- 🛡️ **Real-world pragmatism** - Works with all Git clients today, ready for v2 push when ecosystem matures

### **Current Implementation**
- 🚀 **Upload-pack (fetch/clone)**: Full Protocol v2 with all modern features
- 📤 **Receive-pack (push)**: Protocol v1 for universal client compatibility
- 🔧 **Smart detection** - Automatically uses the right protocol for each operation

## 🌟 Protocol v2 Features (Fetch Operations)

### **Modern Fetch Architecture**
- ✅ **Single service endpoint** with command-based operations
- ✅ **Structured packet-line responses** with clear section headers
- ✅ **Capability-driven negotiation** for optimal performance
- ✅ **Elimination of unnecessary ref advertisements**
- ✅ **Sideband multiplexing** for progress and error reporting

### **Protocol v2 Commands**
- 🔍 **ls-refs** - Smart reference listing with filtering
- 📦 **fetch** - Efficient packfile transfer with negotiation
- ℹ️ **object-info** - Object metadata queries

### **Performance Benefits (Fetch)**
- 🚀 **Faster clone operations** - no full ref advertisement
- 💾 **Reduced bandwidth** - only transfer requested refs
- 🔧 **Better extensibility** - easy to add new capabilities
- 🎯 **Optimized negotiation** - smarter want/have exchanges

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│         Hybrid Git Protocol Server                 │
├─────────────────────────────────────────────────────┤
│  📥 Fetch Operations (Protocol v2)                 │
│    ├── ls-refs    (reference discovery)            │
│    ├── fetch      (packfile transfer)              │
│    └── object-info (metadata queries)              │
│                                                     │
│  📤 Push Operations (Protocol v1)                  │
│    ├── ref advertisement (compatibility)           │
│    ├── ref updates (reliable parsing)              │
│    └── pack processing (proven format)             │
│                                                     │
│  🎯 Smart Protocol Detection                       │
│    ├── v2 for upload-pack endpoints                │
│    └── v1 for receive-pack endpoints               │
│                                                     │
│  🏗️ Theater Actor Integration                      │
│    ├── WebAssembly component execution             │
│    ├── HTTP server management                      │
│    └── State persistence                           │
└─────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### **Build the Component**
```bash
cd git-server
cargo component build --release
```

### **Start the Server**
```bash
theater start manifest.toml
```

### **Test Operations**
```bash
# Clone uses Protocol v2 (fast!)
git clone http://localhost:8080 test-repo

# Push uses Protocol v1 (compatible!)
cd test-repo
echo "test" > file.txt
git add file.txt
git commit -m "test commit"
git push origin main

# Check capabilities
curl "http://localhost:8080/info/refs?service=git-upload-pack"   # v2 response
curl "http://localhost:8080/info/refs?service=git-receive-pack"  # v1 response
```

## 🔬 Protocol Details

### **Fetch Operations (Protocol v2)**
```
# Capability advertisement (v2)
0012version 2
0018agent=git-server/0.1.0
0015object-format=sha1
000eserver-option
0033ls-refs=symrefs peel ref-prefix unborn
0055fetch=shallow thin-pack no-progress include-tag ofs-delta sideband-all wait-for-done
0012object-info=size
0000
```

### **Push Operations (Protocol v1)**
```bash
# Push capability advertisement (v1)
curl "http://localhost:8080/info/refs?service=git-receive-pack"
# Returns traditional v1 format with refs + capabilities
```

### **Testing Both Protocols**
```bash
# Test v2 fetch capabilities
curl -X POST -H "Content-Type: application/x-git-upload-pack-request" \
  --data-binary $'0012command=ls-refs\n0000' \
  http://localhost:8080/git-upload-pack

# Test v1 push compatibility  
git push http://localhost:8080 main  # Uses v1 automatically
```

## 🛠️ Development

### **Project Structure**
```
git-server/
├── src/
│   ├── lib.rs              # Main actor with hybrid routing
│   ├── protocol/
│   │   ├── http.rs         # Protocol v1 push + v2 fetch
│   │   ├── http_fix.rs     # Enhanced v2 implementation
│   │   ├── pack.rs         # Pack file generation
│   │   └── protocol_v2_parser.rs  # v2 command parsing
│   ├── git/
│   │   ├── objects.rs      # Git object types
│   │   └── repository.rs   # Repository state
│   └── utils/              # Hash, compression utilities
├── manifest.toml           # Theater configuration
└── README.md              # This file
```

### **Key Protocol Handlers**

#### **Hybrid Service Detection**
```rust
match service {
    "git-upload-pack" => handle_upload_pack_info_refs(),    // v2
    "git-receive-pack" => handle_receive_pack_info_refs_v1(), // v1
}
```

#### **Protocol v2 Fetch Commands**
- **handle_ls_refs** - Reference discovery with filtering
- **handle_fetch** - Full negotiation with sideband support
- **handle_object_info** - Object metadata queries

#### **Protocol v1 Push Processing**
- **parse_v1_receive_pack_request** - Traditional ref update parsing
- **handle_v1_push** - Reliable pack processing and ref updates

## 🎯 Benefits of Hybrid Approach

### **For Developers**
- 🔧 **Best of both worlds** - v2 performance for reads, v1 reliability for writes
- 📈 **Future-ready** - Easy migration to v2 push when ecosystem stabilizes
- 🚀 **No compatibility issues** - Works with all Git clients today
- 🎨 **Gradual adoption** - Can switch to v2 push per-operation when ready

### **For Git Operations**
- ⚡ **Fast clones** - Protocol v2 fetch with no unnecessary downloads
- 🛡️ **Reliable pushes** - Protocol v1 push with maximum compatibility
- 💾 **Optimized bandwidth** - v2 efficiency where it matters most
- 📊 **Universal support** - Works with legacy and modern Git clients

## 🧪 Testing

### **Debug Endpoints**
```bash
# Server info
curl http://localhost:8080/

# Repository refs  
curl http://localhost:8080/refs

# Git objects
curl http://localhost:8080/objects
```

### **Protocol Testing**
```bash
# Test hybrid protocols
git clone http://localhost:8080 test-repo    # v2 fetch
cd test-repo && git push origin main         # v1 push

# Check protocol responses
curl "http://localhost:8080/info/refs?service=git-upload-pack"   # v2
curl "http://localhost:8080/info/refs?service=git-receive-pack"  # v1
```

## 🔮 Migration Path

### **Current State (Hybrid)**
- ✅ Protocol v2 fetch (stable, fast)
- ✅ Protocol v1 push (compatible, reliable)

### **Future Evolution**
- 🔄 **Monitor v2 push adoption** in Git ecosystem
- 🎯 **Add v2 push detection** when client support stabilizes  
- 🚀 **Gradual migration** - maintain v1 fallback during transition
- 📊 **Performance metrics** to validate v2 push benefits

### **Migration Triggers**
- Git client v2 push support reaches 95%+ adoption
- Major hosting platforms (GitHub, GitLab) fully support v2 push
- Performance benefits clearly outweigh compatibility risks

## 💡 Why This Approach Works

### **Protocol v2 for Fetch (Proven)**
- Mature specification with wide client support
- Significant performance improvements for clone/pull
- Well-tested in production environments

### **Protocol v1 for Push (Pragmatic)**
- Universal client compatibility
- Stable, well-understood protocol
- Proven reliability for critical write operations

### **Best of Both Worlds**
- Get v2 performance benefits where they're most impactful
- Maintain compatibility where it's most critical
- Easy evolution path as v2 push support matures

## 📚 Resources

- [Git Protocol v2 Specification](https://git-scm.com/docs/protocol-v2)
- [Git Protocol v1 Documentation](https://git-scm.com/docs/pack-protocol)
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)
- [Theater Actor System](https://github.com/colinrozzi/theater)

---

**Built with pragmatic technologies:** Rust + WebAssembly + Theater Actors + Hybrid Git Protocols

*Real-world compatibility meets cutting-edge performance* 🚀