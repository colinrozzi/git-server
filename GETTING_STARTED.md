# Getting Started - Git Server Actor

🚀 **5-minute guide to get the WebAssembly git server running**

## What This Is

A **git remote server** (like GitHub, but yours) that runs as a **WebAssembly component** in the **Theater actor system**. Real git clients can clone from it!

## Prerequisites

You need:
- **Rust** (any recent version)
- **Theater CLI** tools
- **5 minutes** ⏰

## Installation

```bash
# 1. Install Rust tools for WebAssembly
rustup target add wasm32-wasip1
cargo install cargo-component

# 2. Install Theater CLI
cargo install theater-server-cli theater-cli

# 3. Get the code
git clone https://github.com/colinrozzi/actor-registry.git
cd actor-registry/git-server
```

## Build & Run

```bash
# 1. Build the WebAssembly component
cargo component build --release

# 2. Start Theater server (keep this running)
theater-server --log-stdout &

# 3. Start the git server actor
theater start manifest.toml
```

## Test It Works

```bash
# Test 1: Basic info
curl http://localhost:8080/
# Should show: Git Repository: git-server

# Test 2: Git discovery (the magic!)
git ls-remote http://localhost:8080
# Should show: 0000000000000000000000000000000000000000	refs/heads/main

# Test 3: Try cloning (will partially work)
git clone http://localhost:8080 test-repo
# Gets through discovery, fails on pack negotiation
```

## What You Just Did

🎉 **Congratulations!** You just:

1. **Built a WebAssembly component** that implements the Git Smart HTTP Transport Protocol
2. **Started a Theater actor** that manages the git server lifecycle  
3. **Tested with real git clients** - your server spoke actual git protocol!

The clone fails because we haven't implemented pack file generation yet - **that's the next big contribution opportunity!**

## Next Steps

- **[Read the README](README.md)** - Full overview and architecture
- **[Check CONTRIBUTING.md](CONTRIBUTING.md)** - detailed contribution guide
- **[Study ARCHITECTURE.md](ARCHITECTURE.md)** - deep technical details
- **[Try modifying the code](src/lib.rs)** - add your own endpoints!

## Quick Code Tour

The main files:
- **`src/lib.rs`** - Main actor implementation (300 lines)
- **`manifest.toml`** - Theater configuration  
- **`Cargo.toml`** - Rust project configuration

Key functions in `src/lib.rs`:
- **`init()`** - Sets up HTTP server and routes
- **`handle_request()`** - Routes HTTP requests  
- **`handle_info_refs()`** - Git discovery (✅ working)
- **`handle_upload_pack()`** - Git clone data (🚧 needs work)

## Troubleshooting

**"Component won't build"**:
```bash
cargo clean
cargo component build --release
```

**"Theater server won't start"**:
```bash
# Check if already running
ps aux | grep theater
# Kill if needed, then restart
theater-server --log-stdout
```

**"Git client can't connect"**:
```bash
# Test HTTP directly
curl -v http://localhost:8080/
# Check theater status
theater list
```

## What Makes This Special

- **Real git protocol** - Works with standard git clients
- **WebAssembly security** - Sandboxed execution environment
- **Actor supervision** - Automatic restart and monitoring  
- **Event chain** - Complete audit log of all operations
- **Modern architecture** - Built for cloud-native environments

## Ready to Contribute?

The **biggest impact** you can make right now:

1. **Implement pack protocol** in `handle_upload_pack()` 
2. **Add real git objects** instead of empty repository
3. **Create web interface** for browsing repositories
4. **Improve documentation** and examples

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidance!

---

**Welcome to the future of distributed version control!** 🌟
