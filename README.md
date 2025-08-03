# Git Server Actor - Protocol v2 🚀

**A cutting-edge WebAssembly Git server implementing Git Wire Protocol v2**

This project has been **completely modernized** to use only Git Protocol v2 - the latest and most efficient Git protocol specification. No legacy v1 support means cleaner, faster, and more maintainable code.

## 🌟 What's New in Protocol v2

### **Modern Architecture**
- ✅ **Single service endpoint** with command-based operations
- ✅ **Structured packet-line responses** with clear section headers
- ✅ **Capability-driven negotiation** for optimal performance
- ✅ **Elimination of unnecessary ref advertisements**
- ✅ **Sideband multiplexing** for progress and error reporting

### **Protocol v2 Commands**
- 🔍 **ls-refs** - Smart reference listing with filtering
- 📦 **fetch** - Efficient packfile transfer with negotiation
- ℹ️ **object-info** - Object metadata queries

### **Performance Benefits**
- 🚀 **Faster clone operations** - no full ref advertisement
- 💾 **Reduced bandwidth** - only transfer requested refs
- 🔧 **Better extensibility** - easy to add new capabilities
- 🎯 **Optimized negotiation** - smarter want/have exchanges

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│              Git Protocol v2 Server                │
├─────────────────────────────────────────────────────┤
│  🎯 Command Router                                  │
│    ├── ls-refs    (reference discovery)            │
│    ├── fetch      (packfile transfer)              │
│    └── object-info (metadata queries)              │
│                                                     │
│  📦 Packet-Line Protocol v2                        │
│    ├── Capability advertisement                    │
│    ├── Structured responses                        │
│    └── Sideband multiplexing                       │
│                                                     │
│  🏗️ Theater Actor Integration                       │
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

### **Test Protocol v2**
```bash
# Modern Git clients automatically use v2
git clone http://localhost:8080 test-repo

# Force v2 (for older Git versions)
git -c protocol.version=2 clone http://localhost:8080 test-repo

# Test capabilities
curl "http://localhost:8080/info/refs?service=git-upload-pack"
```

## 🔬 Protocol v2 Features

### **Capability Advertisement**
```
0012version 2
0018agent=git-server/0.1.0
0015object-format=sha1
000eserver-option
0033ls-refs=symrefs peel ref-prefix unborn
0055fetch=shallow thin-pack no-progress include-tag ofs-delta sideband-all wait-for-done
0012object-info=size
0000
```

### **ls-refs Command**
```bash
# List all references
curl -X POST -H "Content-Type: application/x-git-upload-pack-request" \
  --data-binary $'0012command=ls-refs\n0000' \
  http://localhost:8080/git-upload-pack

# List refs with prefix filtering
curl -X POST -H "Content-Type: application/x-git-upload-pack-request" \
  --data-binary $'0012command=ls-refs\n001eref-prefix refs/heads/\n0000' \
  http://localhost:8080/git-upload-pack
```

### **fetch Command Structure**
```
command=fetch
want <oid>
have <oid>
done
↓
acknowledgments
ACK <oid>
ready
0001
packfile
<sideband-multiplexed-pack-data>
0000
```

## 🛠️ Development

### **Project Structure**
```
git-server/
├── src/
│   ├── lib.rs              # Main actor with Protocol v2 routing
│   ├── protocol/
│   │   ├── http.rs         # Protocol v2 implementation
│   │   └── pack.rs         # Pack file generation
│   ├── git/
│   │   ├── objects.rs      # Git object types
│   │   └── repository.rs   # Repository state
│   └── utils/              # Hash, compression utilities
├── manifest.toml           # Theater configuration
└── README.md              # This file
```

### **Key Protocol v2 Handlers**

#### **handle_smart_info_refs**
- Generates capability advertisement
- Announces Protocol v2 support
- Lists available commands and features

#### **handle_upload_pack_request**  
- Routes Protocol v2 commands
- Parses packet-line requests
- Executes ls-refs, fetch, object-info

#### **handle_ls_refs_command**
- Reference discovery with filtering
- Supports symrefs, peeled tags
- Prefix-based ref selection

#### **handle_fetch_command**
- Full negotiation protocol
- Structured response sections
- Sideband pack transmission

## 🎯 Protocol v2 Benefits

### **For Developers**
- 🔧 **Cleaner codebase** - single protocol implementation
- 📈 **Better performance** - optimized for modern workflows  
- 🚀 **Future-ready** - foundation for upcoming Git features
- 🎨 **Extensible design** - easy to add new commands

### **For Git Operations**
- ⚡ **Faster clones** - no unnecessary ref downloads
- 💾 **Reduced bandwidth** - only transfer what's needed
- 🎯 **Smarter negotiation** - better want/have exchanges
- 📊 **Better progress** - sideband status reporting

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
# Run comprehensive tests
./test_v2.sh

# Test with real Git client
git -c protocol.version=2 ls-remote http://localhost:8080
git -c protocol.version=2 clone http://localhost:8080 test-repo
```

## 🔮 Future Enhancements

With Protocol v2 foundation in place:

- 🔄 **Push operations** via fetch command variants
- 🎯 **Partial clone** support with object filters  
- 📊 **Advanced object-info** attributes
- 🔗 **Custom capabilities** for specialized workflows
- 🚀 **Performance optimizations** with modern Git features

## 📚 Protocol v2 Resources

- [Git Protocol v2 Specification](https://git-scm.com/docs/protocol-v2)
- [Git Pack Protocol](https://git-scm.com/docs/pack-protocol)
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)
- [Theater Actor System](https://github.com/colinrozzi/theater)

---

**Built with modern technologies:** Rust + WebAssembly + Theater Actors + Git Protocol v2

*No legacy protocol support = cleaner code, better performance, future-ready architecture* 🚀
