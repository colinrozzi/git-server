# Contributing to Git Server Actor

Welcome! This guide will help you get up to speed and start contributing to this exciting WebAssembly git server project.

## 🚀 Quick Start for Contributors

### **What You're Joining**
You're contributing to a **groundbreaking project**: a fully functional git remote server that runs as a WebAssembly component in the Theater actor system. We've already achieved:

- ✅ **Real git client compatibility** - `git ls-remote` works!
- ✅ **HTTP protocol implementation** - Complete discovery phase
- ✅ **WebAssembly actor architecture** - Production-ready foundation
- 🚧 **Pack protocol in progress** - The next major milestone

### **Current Status**
```bash
# This works right now! 🎉
git ls-remote http://localhost:8080
0000000000000000000000000000000000000000	refs/heads/main

# This gets 95% of the way there! 🚧
git clone http://localhost:8080 test-repo  
# Fails on pack negotiation - this is where we need help!
```

## 🛠️ Development Setup

### **Prerequisites**
```bash
# Install Rust with WebAssembly support
rustup target add wasm32-wasip1
cargo install cargo-component

# Install Theater CLI tools
cargo install theater-server-cli theater-cli

# Clone the actor registry
git clone https://github.com/colinrozzi/actor-registry.git
cd actor-registry/git-server
```

### **Build and Test**
```bash
# Build the WebAssembly component
cargo component build --release

# Start Theater server (in another terminal)
theater-server --log-stdout

# Start the git server actor
theater start manifest.toml

# Test it works!
git ls-remote http://localhost:8080
curl http://localhost:8080/
```

### **Development Workflow**
```bash
# Make your changes
vim src/lib.rs

# Rebuild and test
cargo component build --release
theater stop <actor-id>  # Get ID from: theater list
theater start manifest.toml

# Test your changes
curl "http://localhost:8080/info/refs?service=git-upload-pack"
git ls-remote http://localhost:8080
```

## 🎯 Contribution Opportunities

### **🔥 High Priority: Complete Git Clone Support**

**The Challenge**: We need to implement pack protocol negotiation in `handle_upload_pack()`

**Current State**:
```rust
fn handle_upload_pack(_repo_state: &mut GitRepoState, _request: &HttpRequest) -> HttpResponse {
    // Returns empty pack - git client expects ACK/NAK + real pack data
    let response_body = b"0000"; 
    create_response(200, "application/x-git-upload-pack-result", response_body)
}
```

**What's Needed**:
1. **Parse want/have negotiation** from request body
2. **Generate ACK/NAK responses** based on available objects  
3. **Create pack files** with requested objects
4. **Add real git objects** to make cloning meaningful

**Skills Needed**: Rust, Git protocol knowledge, binary parsing
**Impact**: Make git clone fully work! 🚀
**Difficulty**: Medium-Hard

### **🛠️ Medium Priority: Repository Content**

**Add Real Git Objects**:
```rust
// Instead of empty repository, create real commits
let initial_commit = GitObject::Commit {
    tree: "tree_hash_here",
    parents: vec![],
    author: "Git Server Actor <git@theater.dev>".to_string(),
    committer: "Git Server Actor <git@theater.dev>".to_string(), 
    message: "Initial commit from WebAssembly git server".to_string(),
};
```

**Create File Content**:
```rust
// Add actual files to the repository
let readme_blob = GitObject::Blob {
    content: b"# Hello from WebAssembly Git Server!\n\nThis repository is served by a Theater actor.".to_vec(),
};
```

**Skills Needed**: Rust, Git internals understanding
**Impact**: Make the repository actually contain something useful
**Difficulty**: Medium

### **🌐 Medium Priority: Web Interface**

**Add Repository Browser**:
- `GET /browse` - HTML page showing repository contents  
- `GET /commits` - Commit history view
- `GET /files/{path}` - File browser
- CSS styling for a modern look

**Skills Needed**: HTML/CSS, Rust HTTP handling
**Impact**: Make the git server accessible via web browser
**Difficulty**: Easy-Medium

### **🔒 Future: Authentication & Authorization**

**Add Git Authentication**:
- HTTP Basic Auth support
- Token-based authentication
- Per-repository permissions
- Integration with Theater's security model

**Skills Needed**: Security, HTTP authentication, Theater actors
**Impact**: Make it production-ready for private repos
**Difficulty**: Hard

### **📊 Easy Wins: Observability & Debugging**

**Enhanced Logging**:
```rust
log(&format!("Git operation: {} from client {}", operation, client_ip));
```

**Metrics Collection**:
- Count of clone/push operations
- Repository access patterns
- Performance measurements

**Better Error Messages**:
- More descriptive git protocol errors
- Client troubleshooting information

**Skills Needed**: Rust, basic observability concepts
**Impact**: Make debugging and monitoring easier
**Difficulty**: Easy

## 📚 Learning Resources

### **Understanding Git Protocol**
Start here to understand what we're implementing:

1. **[Git Smart HTTP Transport](https://git-scm.com/docs/http-protocol)** - Official protocol spec
2. **[Git Pack Protocol](https://git-scm.com/docs/pack-protocol)** - Pack file format  
3. **[Packet-Line Format](https://git-scm.com/docs/protocol-common)** - Wire protocol basics

### **Understanding Theater Actors**
Learn the foundation we're building on:

1. **[Theater Documentation](https://colinrozzi.github.io/theater/guide)** - Complete guide
2. **[Actor Registry Examples](https://github.com/colinrozzi/actor-registry)** - Other actor implementations
3. **[WebAssembly Components](https://component-model.bytecodealliance.org/)** - WASM component model

### **Git Internals**
Understand how git works under the hood:

1. **[Pro Git Book - Git Internals](https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain)** - Excellent introduction
2. **[Git Source Code](https://github.com/git/git)** - Reference implementation
3. **Study existing implementations**: libgit2, go-git, dulwich

## 🐛 Debugging Guide

### **Common Development Issues**

**"Component won't build"**:
```bash
# Check Rust version
rustup show

# Ensure wasm target installed  
rustup target add wasm32-wasip1

# Clean and rebuild
cargo clean
cargo component build --release
```

**"Actor won't start"**:
```bash
# Check Theater server is running
theater list

# Check manifest points to correct WASM file  
cat manifest.toml

# Look at detailed logs
theater-server --log-stdout
```

**"Git client can't connect"**:
```bash
# Test HTTP server directly
curl -v http://localhost:8080/

# Check port is available
lsof -i :8080

# Test git protocol endpoint
curl "http://localhost:8080/info/refs?service=git-upload-pack"
```

### **Debugging Tools**

**Theater CLI**:
```bash
theater list                    # Show all actors
theater events <actor-id>       # View actor event chain  
theater stop <actor-id>         # Stop actor
theater start manifest.toml     # Start actor
```

**Git Debugging**:
```bash
# Verbose git client output
git -c http.verbose=true ls-remote http://localhost:8080

# Manual HTTP requests
curl -v "http://localhost:8080/info/refs?service=git-upload-pack"

# Capture network traffic  
wireshark  # or tcpdump
```

**WebAssembly Inspection**:
```bash
# Validate WASM component
wasm-validate target/wasm32-wasip1/release/git_server.wasm

# Inspect component structure  
wasm-objdump -x target/wasm32-wasip1/release/git_server.wasm
```

## 📋 Code Style Guidelines

### **Rust Code Style**
```bash
# Format code
cargo fmt

# Lint code  
cargo clippy

# Run tests
cargo test
```

### **Git Protocol Implementation**
- **Follow official specs** - Reference git documentation for all protocol details
- **Test with real clients** - Always test with actual git command-line client
- **Handle edge cases** - Git protocol has many special cases and error conditions
- **Maintain compatibility** - Support multiple git client versions

### **Theater Actor Patterns**
- **Proper error handling** - Use Theater's error patterns
- **State management** - Serialize/deserialize state correctly
- **Resource cleanup** - Clean up resources on actor shutdown
- **Event logging** - Log important operations for observability

### **Documentation**
- **Code comments** - Explain complex git protocol logic
- **Update README** - Keep user documentation current
- **Architecture docs** - Update ARCHITECTURE.md for significant changes
- **Examples** - Include usage examples in documentation

## 🤝 Pull Request Process

### **Before You Start**
1. **Check existing issues** - See if someone is already working on it
2. **Open an issue** - Discuss your approach before large changes
3. **Start small** - Begin with small, focused contributions

### **Development Process**
1. **Fork the repository** - Create your own fork to work in
2. **Create feature branch** - `git checkout -b feature/pack-protocol`
3. **Make incremental commits** - Small, focused commits with good messages
4. **Test thoroughly** - Test with real git clients, not just unit tests
5. **Update documentation** - Include relevant documentation updates

### **Pull Request Guidelines**
- **Clear description** - Explain what you changed and why
- **Test results** - Show that `git ls-remote` and other operations work
- **Breaking changes** - Call out any breaking changes clearly
- **Screenshots** - Include screenshots for UI changes

### **Review Process**
- **Code review** - Maintainers will review for correctness and style
- **Testing** - Changes will be tested with real git clients
- **Integration** - Ensure changes work with existing Theater infrastructure
- **Documentation** - Verify documentation is updated appropriately

## 🏆 Recognition

Contributors will be recognized in:
- **README.md** - Listed as contributors
- **Git commit history** - Your commits are part of the permanent record
- **Theater community** - Shared in Theater project communications
- **Conference talks** - This project may be presented at conferences

## 💬 Getting Help

### **Community**
- **GitHub Issues** - Ask questions, report bugs, suggest features
- **Theater Documentation** - Official Theater docs and guides
- **Code Comments** - Extensive inline documentation in the codebase

### **Mentorship**
If you're new to:
- **Git Protocol** - Start with simple debugging endpoints, work up to pack protocol
- **WebAssembly** - Focus on Rust code first, WASM compilation is mostly automated
- **Theater Actors** - Study existing actors in the registry for patterns
- **Rust** - This is a great project to learn Rust with real-world impact!

### **Getting Unstuck**
- **Read the tests** - Look at existing test cases for examples
- **Compare with git source** - See how git itself implements the protocol
- **Use debugging tools** - Theater provides excellent observability
- **Ask questions** - Open GitHub issues for help, no question is too basic

## 🎯 Roadmap & Vision

### **Short Term (Next 1-2 months)**
- [ ] Complete pack protocol implementation
- [ ] Add real git objects and commits
- [ ] Implement git push support  
- [ ] Add comprehensive test suite

### **Medium Term (3-6 months)**
- [ ] Web interface for repository browsing
- [ ] Authentication and authorization
- [ ] Multiple repository support
- [ ] Performance optimizations

### **Long Term (6+ months)**
- [ ] Git LFS (Large File Storage) support
- [ ] Webhooks and CI/CD integration
- [ ] Repository mirroring and backup
- [ ] Distributed multi-actor architecture

### **Vision**
This project demonstrates the future of infrastructure software:
- **WebAssembly** for security and portability
- **Actor systems** for reliability and scalability  
- **Event chains** for complete observability
- **Modern protocols** built on proven foundations

## 🌟 Why This Matters

### **Technical Innovation**
- **First-ever** WebAssembly git server
- **Production-ready** actor system architecture
- **Complete observability** with event chains
- **Modern security** with WASM sandboxing

### **Real-World Impact**
- **Distributed development** - Git servers that can run anywhere
- **Secure infrastructure** - Sandboxed, auditable version control
- **Cloud-native** - Built for container and serverless environments
- **Developer experience** - Standard git clients work unchanged

### **Learning Opportunity**
- **Deep dive** into git protocol internals
- **Hands-on** WebAssembly development
- **Modern** distributed systems patterns
- **Production** software development practices

## 🚀 Ready to Contribute?

1. **Set up your environment** - Follow the development setup above
2. **Pick an issue** - Start with "good first issue" labels
3. **Join the community** - Introduce yourself in GitHub discussions
4. **Start coding** - Make your first contribution!

**Welcome to the future of version control infrastructure!** 🎉

---

*This project is part of the broader Theater ecosystem, pushing the boundaries of what's possible with WebAssembly and actor systems. Your contributions help build the foundation for the next generation of distributed software.*
