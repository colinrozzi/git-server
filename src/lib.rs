// Simple SHA-1 implementation for Git pack checksums
fn sha1_hash(data: &[u8]) -> [u8; 20] {
    let mut h = [
        0x67452301u32,
        0xEFCDAB89u32,
        0x98BADCFEu32,
        0x10325476u32,
        0xC3D2E1F0u32,
    ];
    
    let original_len = data.len();
    let mut message = data.to_vec();
    
    // Append the '1' bit (plus zero padding to make it a byte)
    message.push(0x80);
    
    // Append zeros until length ≡ 448 (mod 512)
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    
    // Append original length in bits as 64-bit big-endian integer
    let bit_len = (original_len as u64) * 8;
    message.extend(&bit_len.to_be_bytes());
    
    // Process message in 512-bit chunks
    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 80];
        
        // Break chunk into sixteen 32-bit big-endian words
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1], 
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        
        // Extend the words
        for i in 16..80 {
            w[i] = left_rotate(w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16], 1);
        }
        
        // Initialize hash value for this chunk
        let [mut a, mut b, mut c, mut d, mut e] = h;
        
        // Main loop
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                60..=79 => (b ^ c ^ d, 0xCA62C1D6),
                _ => unreachable!(),
            };
            
            let temp = left_rotate(a, 5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            
            e = d;
            d = c;
            c = left_rotate(b, 30);
            b = a;
            a = temp;
        }
        
        // Add this chunk's hash to result
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    
    // Convert to bytes
    let mut result = [0u8; 20];
    for (i, &word) in h.iter().enumerate() {
        let bytes = word.to_be_bytes();
        result[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    
    result
}

fn left_rotate(value: u32, bits: u32) -> u32 {
    (value << bits) | (value >> (32 - bits))
}

#[allow(warnings)]
mod bindings;

use bindings::exports::theater::simple::actor::Guest;
use bindings::exports::theater::simple::http_handlers::Guest as HttpHandlers;
use bindings::theater::simple::runtime::log;
use bindings::theater::simple::http_framework::{self};
use bindings::theater::simple::http_types::ServerConfig;
use bindings::theater::simple::http_types::{HttpRequest, HttpResponse, MiddlewareResult};
use bindings::exports::theater::simple::http_handlers::{HandlerId, WebsocketMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GitRepoState {
    repo_name: String,
    
    // Git references (branches/tags) -> commit hash
    refs: HashMap<String, String>,
    
    // Git objects: hash -> object data
    objects: HashMap<String, GitObject>,
    
    // HEAD reference (usually "refs/heads/main")
    head: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum GitObject {
    Blob { content: Vec<u8> },
    Tree { entries: Vec<TreeEntry> },
    Commit { 
        tree: String,
        parents: Vec<String>,
        author: String,
        committer: String,
        message: String,
    },
    Tag { 
        object: String,
        tag_type: String,
        tagger: String,
        message: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TreeEntry {
    mode: String,
    name: String,
    hash: String,
}

impl Default for GitRepoState {
    fn default() -> Self {
        let mut refs = HashMap::new();
        refs.insert("refs/heads/main".to_string(), "0000000000000000000000000000000000000000".to_string());
        
        Self {
            repo_name: "git-server".to_string(),
            refs,
            objects: HashMap::new(),
            head: "refs/heads/main".to_string(),
        }
    }
}

// Utility function to create minimal repository objects
fn ensure_minimal_repo_objects(repo_state: &mut GitRepoState) {
    // If we don't have any real objects, create some basic ones
    if repo_state.objects.is_empty() {
        log("Creating minimal repository objects");
        
        // Create a simple blob (README file)
        let readme_content = b"# Git Server\n\nThis is a WebAssembly git server!\n";
        let readme_hash = calculate_git_hash("blob", readme_content);
        repo_state.objects.insert(
            readme_hash.clone(),
            GitObject::Blob {
                content: readme_content.to_vec(),
            },
        );
        log(&format!("Created blob: {}", readme_hash));
        
        // Create a tree containing the README
        let tree_entries = vec![TreeEntry {
            mode: "100644".to_string(),
            name: "README.md".to_string(),
            hash: readme_hash,
        }];
        let tree_content = serialize_tree_object(&tree_entries);
        let tree_hash = calculate_git_hash("tree", &tree_content);
        repo_state.objects.insert(
            tree_hash.clone(),
            GitObject::Tree {
                entries: tree_entries,
            },
        );
        log(&format!("Created tree: {}", tree_hash));
        
        // Create a commit pointing to the tree
        let commit_message = "Initial commit";
        let author = "Git Server <git-server@example.com>";
        let committer = author;
        
        let commit_content = serialize_commit_object(&tree_hash, &[], author, committer, commit_message);
        let commit_hash = calculate_git_hash("commit", &commit_content);
        repo_state.objects.insert(
            commit_hash.clone(),
            GitObject::Commit {
                tree: tree_hash,
                parents: vec![],
                author: author.to_string(),
                committer: committer.to_string(),
                message: commit_message.to_string(),
            },
        );
        log(&format!("Created commit: {}", commit_hash));
        
        // Update refs to point to the new commit
        repo_state.refs.insert("refs/heads/main".to_string(), commit_hash.clone());
        log(&format!("Updated refs/heads/main to: {}", commit_hash));
        
        log(&format!("Created {} objects with proper SHA-1 hashes", repo_state.objects.len()));
    }
}

struct Component;

impl Guest for Component {
    fn init(
        state: Option<Vec<u8>>,
        params: (String,),
    ) -> Result<(Option<Vec<u8>>,), String> {
        log("Initializing git-server actor");
        let (self_id,) = params;
        log(&format!("Git server actor ID: {}", &self_id));

        // Parse existing state or create new
        let mut repo_state = match state {
            Some(bytes) => {
                serde_json::from_slice::<GitRepoState>(&bytes)
                    .unwrap_or_else(|_| {
                        log("Failed to parse existing state, creating new");
                        GitRepoState::default()
                    })
            }
            None => {
                log("No existing state, creating new git repository");
                GitRepoState::default()
            }
        };

        // Ensure we have some basic objects from the start
        ensure_minimal_repo_objects(&mut repo_state);
        
        log(&format!("Git repository '{}' initialized with {} refs and {} objects", 
                     repo_state.repo_name, 
                     repo_state.refs.len(),
                     repo_state.objects.len()));

        // Set up HTTP server for git protocol
        let config = ServerConfig {
            port: Some(8080),
            host: Some("0.0.0.0".to_string()),
            tls_config: None,
        };

        // Create the server
        let server_id = http_framework::create_server(&config)
            .map_err(|e| format!("Failed to create HTTP server: {}", e))?;

        // Register a git handler with explicit error handling
        let git_handler = match http_framework::register_handler("git") {
            Ok(handler_id) => {
                log(&format!("Successfully registered git handler with ID: {}", handler_id));
                handler_id
            }
            Err(e) => {
                log(&format!("Failed to register git handler: {}", e));
                return Err(format!("Failed to register git handler: {}", e));
            }
        };
        
        log(&format!("Using git handler ID: {}", git_handler));
        
        // Add a small delay to ensure handler is fully registered
        // (This might help with timing issues)
        
        // Add git protocol routes one by one with proper error handling
        match http_framework::add_route(server_id, "/info/refs", "GET", git_handler) {
            Ok(_) => log("Added GET /info/refs route"),
            Err(e) => {
                log(&format!("Failed to add /info/refs route: {}", e));
                return Err(format!("Failed to add /info/refs route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/git-upload-pack", "POST", git_handler) {
            Ok(_) => log("Added POST /git-upload-pack route"),
            Err(e) => {
                log(&format!("Failed to add /git-upload-pack route: {}", e));
                return Err(format!("Failed to add /git-upload-pack route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/git-receive-pack", "POST", git_handler) {
            Ok(_) => log("Added POST /git-receive-pack route"),
            Err(e) => {
                log(&format!("Failed to add /git-receive-pack route: {}", e));
                return Err(format!("Failed to add /git-receive-pack route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/", "GET", git_handler) {
            Ok(_) => log("Added GET / route"),
            Err(e) => {
                log(&format!("Failed to add / route: {}", e));
                return Err(format!("Failed to add / route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/refs", "GET", git_handler) {
            Ok(_) => log("Added GET /refs route"),
            Err(e) => {
                log(&format!("Failed to add /refs route: {}", e));
                return Err(format!("Failed to add /refs route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/objects", "GET", git_handler) {
            Ok(_) => log("Added GET /objects route"),
            Err(e) => {
                log(&format!("Failed to add /objects route: {}", e));
                return Err(format!("Failed to add /objects route: {}", e));
            }
        }

        // Start the server
        let actual_port = http_framework::start_server(server_id)
            .map_err(|e| format!("Failed to start HTTP server: {}", e))?;

        log(&format!("Git server started on port {}", actual_port));

        // Serialize state back
        let new_state = serde_json::to_vec(&repo_state)
            .map_err(|e| format!("Failed to serialize state: {}", e))?;

        Ok((Some(new_state),))
    }
}

impl HttpHandlers for Component {
    fn handle_request(
        state: Option<Vec<u8>>,
        params: (HandlerId, HttpRequest),
    ) -> Result<(Option<Vec<u8>>, (HttpResponse,)), String> {
        let (_handler_id, request) = params;
        
        log(&format!("HTTP {} {}", request.method, request.uri));
        
        // Parse current state
        let mut repo_state: GitRepoState = match state {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Failed to parse state: {}", e))?,
            None => GitRepoState::default(),
        };

        // Route the request
        let response = match (request.method.as_str(), request.uri.as_str()) {
            // Git Smart HTTP Protocol endpoints
            ("GET", uri) if uri.contains("/info/refs") => {
                handle_info_refs(&repo_state, &request)
            }
            ("POST", uri) if uri.ends_with("/git-upload-pack") => {
                handle_upload_pack(&mut repo_state, &request)
            }
            ("POST", uri) if uri.ends_with("/git-receive-pack") => {
                handle_receive_pack(&mut repo_state, &request)
            }
            
            // Debug/info endpoints for development
            ("GET", "/") => {
                handle_repo_info(&repo_state)
            }
            ("GET", "/refs") => {
                handle_list_refs(&repo_state)
            }
            ("GET", "/objects") => {
                handle_list_objects(&repo_state)
            }
            
            // 404 for everything else
            _ => {
                log(&format!("Unknown route: {} {}", request.method, request.uri));
                create_response(404, "text/plain", "Not Found".as_bytes())
            }
        };

        // Serialize updated state
        let new_state = serde_json::to_vec(&repo_state)
            .map_err(|e| format!("Failed to serialize updated state: {}", e))?;

        Ok((Some(new_state), (response,)))
    }

    fn handle_middleware(
        state: Option<Vec<u8>>,
        params: (HandlerId, HttpRequest),
    ) -> Result<(Option<Vec<u8>>, (MiddlewareResult,)), String> {
        let (_handler_id, request) = params;
        // For now, just pass through all requests
        let middleware_result = MiddlewareResult {
            proceed: true,
            request,
        };
        Ok((state, (middleware_result,)))
    }

    fn handle_websocket_connect(
        state: Option<Vec<u8>>,
        params: (HandlerId, u64, String, Option<String>),
    ) -> Result<(Option<Vec<u8>>,), String> {
        // Git doesn't use WebSockets, so just accept but do nothing
        Ok((state,))
    }

    fn handle_websocket_message(
        state: Option<Vec<u8>>,
        params: (HandlerId, u64, WebsocketMessage),
    ) -> Result<(Option<Vec<u8>>, (Vec<WebsocketMessage>,)), String> {
        // Git doesn't use WebSockets, return empty response
        Ok((state, (vec![],)))
    }

    fn handle_websocket_disconnect(
        state: Option<Vec<u8>>,
        params: (HandlerId, u64),
    ) -> Result<(Option<Vec<u8>>,), String> {
        // Git doesn't use WebSockets, just acknowledge
        Ok((state,))
    }
}

// Git Smart HTTP Protocol Handlers

fn handle_info_refs(repo_state: &GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Handling /info/refs request");
    
    // Parse query parameters to get the service
    let service = extract_query_param(&request.uri, "service");
    
    match service.as_deref() {
        Some("git-upload-pack") => {
            log("Info/refs for git-upload-pack (clone/fetch)");
            handle_upload_pack_discovery(repo_state)
        }
        Some("git-receive-pack") => {
            log("Info/refs for git-receive-pack (push)");
            handle_receive_pack_discovery(repo_state)
        }
        _ => {
            log(&format!("Unknown service parameter: {:?}", service));
            create_response(400, "text/plain", "Bad Request: missing or invalid service parameter".as_bytes())
        }
    }
}

fn handle_upload_pack_discovery(repo_state: &GitRepoState) -> HttpResponse {
    log("Generating upload-pack advertisement");
    
    let mut response_body = Vec::new();
    
    // Service announcement
    let service_line = "# service=git-upload-pack\n";
    response_body.extend(format_pkt_line(service_line));
    response_body.extend(b"0000"); // Flush packet
    
    // Advertise refs
    for (ref_name, commit_hash) in &repo_state.refs {
        let ref_line = format!("{} {}\n", commit_hash, ref_name);
        response_body.extend(format_pkt_line(&ref_line));
    }
    
    response_body.extend(b"0000"); // End of refs
    
    log(&format!("Upload-pack discovery response: {} bytes", response_body.len()));
    
    create_response(
        200,
        "application/x-git-upload-pack-advertisement",
        &response_body
    )
}

fn handle_receive_pack_discovery(repo_state: &GitRepoState) -> HttpResponse {
    log("Generating receive-pack advertisement");
    
    let mut response_body = Vec::new();
    
    // Service announcement
    let service_line = "# service=git-receive-pack\n";
    response_body.extend(format_pkt_line(service_line));
    response_body.extend(b"0000"); // Flush packet
    
    // Advertise refs with capabilities
    let mut first_ref = true;
    for (ref_name, commit_hash) in &repo_state.refs {
        let ref_line = if first_ref {
            first_ref = false;
            format!("{} {}\0report-status delete-refs side-band-64k\n", commit_hash, ref_name)
        } else {
            format!("{} {}\n", commit_hash, ref_name)
        };
        response_body.extend(format_pkt_line(&ref_line));
    }
    
    response_body.extend(b"0000"); // End of refs
    
    log(&format!("Receive-pack discovery response: {} bytes", response_body.len()));
    
    create_response(
        200,
        "application/x-git-receive-pack-advertisement",
        &response_body
    )
}

fn handle_upload_pack(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Handling upload-pack request (clone/fetch data transfer)");
    
    // Parse the request body to extract want/have lines
    let request_body = request.body.as_ref().map(|b| b.as_slice()).unwrap_or(&[]);
    let negotiation = parse_upload_pack_request(request_body);
    
    log(&format!("Client wants {} objects, has {} objects", 
                 negotiation.wants.len(), 
                 negotiation.haves.len()));
    
    // For each want, check if we have it and determine what to send
    let mut objects_to_send = Vec::new();
    let mut missing_objects = Vec::new();
    
    for want_hash in &negotiation.wants {
        // Zero hash means client wants everything (fresh clone)
        if want_hash == "0000000000000000000000000000000000000000" {
            log("Client wants full clone (zero hash)");
            // Send all our refs
            for (ref_name, ref_hash) in &repo_state.refs {
                if ref_hash != "0000000000000000000000000000000000000000" {
                    objects_to_send.push(ref_hash.clone());
                    log(&format!("Will send ref {}: {}", ref_name, ref_hash));
                }
            }
        } else if repo_state.refs.values().any(|h| h == want_hash) {
            // We have this specific ref, add it to objects to send
            objects_to_send.push(want_hash.clone());
            log(&format!("Will send object: {}", want_hash));
        } else {
            missing_objects.push(want_hash.clone());
            log(&format!("Missing object: {}", want_hash));
        }
    }
    
    // Build the response
    let mut response_body = Vec::new();
    
    // Phase 1: Negotiation response
    // For a first clone, client sends want with zero hashes and no haves
    // We need to respond with NAK and then send pack
    
    if negotiation.haves.is_empty() {
        // First clone - no negotiation needed, client has nothing
        // Send NAK to end negotiation phase  
        response_body.extend(format_pkt_line("NAK\n"));
    } else {
        // Handle have/want negotiation
        let mut found_common = false;
        
        for have_hash in &negotiation.haves {
            if repo_state.refs.values().any(|h| h == have_hash) || repo_state.objects.contains_key(have_hash) {
                let ack_line = format!("ACK {}\n", have_hash);
                response_body.extend(format_pkt_line(&ack_line));
                found_common = true;
            }
        }
        
        if !found_common {
            response_body.extend(format_pkt_line("NAK\n"));
        }
    }
    
    // Always send a pack for clone operations
    log("Generating pack file");
    
    // If no specific objects requested, send all objects (for fresh clone)
    if objects_to_send.is_empty() {
        log("No specific objects requested, sending all objects");
        for (ref_name, ref_hash) in &repo_state.refs {
            if ref_hash != "0000000000000000000000000000000000000000" {
                objects_to_send.push(ref_hash.clone());
                log(&format!("Added ref {} to pack: {}", ref_name, ref_hash));
            }
        }
    }
    
    // Generate and send pack file
    if !objects_to_send.is_empty() {
        let pack_data = generate_pack_file(repo_state, &objects_to_send);
        // For git clone, pack data is sent directly after negotiation (not in packet-line format)
        response_body.extend(pack_data);
    } else {
        log("Still no objects, sending empty pack");
        let empty_pack = generate_empty_pack();
        response_body.extend(empty_pack);
    }
    
    // Don't add flush packet after pack data - Git doesn't expect it
    // The pack data itself is the end of the response
    
    log(&format!("Upload-pack response: {} bytes", response_body.len()));
    
    create_response(
        200,
        "application/x-git-upload-pack-result",
        &response_body
    )
}

fn handle_receive_pack(_repo_state: &mut GitRepoState, _request: &HttpRequest) -> HttpResponse {
    log("Handling receive-pack request (push data transfer)");
    
    // For now, return a minimal response
    // TODO: Parse pack file from request body
    // TODO: Update refs based on push
    
    let response_body = b"0000"; // Empty response for now
    
    create_response(
        200,
        "application/x-git-receive-pack-result",
        response_body
    )
}

// Debug/Development Endpoints

fn handle_repo_info(repo_state: &GitRepoState) -> HttpResponse {
    let info = format!(
        "Git Repository: {}\nHEAD: {}\nRefs: {}\nObjects: {}\n",
        repo_state.repo_name,
        repo_state.head,
        repo_state.refs.len(),
        repo_state.objects.len()
    );
    
    create_response(200, "text/plain", info.as_bytes())
}

fn handle_list_refs(repo_state: &GitRepoState) -> HttpResponse {
    let mut refs_list = String::new();
    refs_list.push_str(&format!("HEAD: {}\n", repo_state.head));
    
    for (ref_name, commit_hash) in &repo_state.refs {
        refs_list.push_str(&format!("{}: {}\n", ref_name, commit_hash));
    }
    
    create_response(200, "text/plain", refs_list.as_bytes())
}

fn handle_list_objects(repo_state: &GitRepoState) -> HttpResponse {
    let mut objects_list = String::new();
    
    for (hash, obj) in &repo_state.objects {
        let obj_type = match obj {
            GitObject::Blob { .. } => "blob",
            GitObject::Tree { .. } => "tree", 
            GitObject::Commit { .. } => "commit",
            GitObject::Tag { .. } => "tag",
        };
        objects_list.push_str(&format!("{}: {}\n", hash, obj_type));
    }
    
    if objects_list.is_empty() {
        objects_list.push_str("No objects in repository\n");
    }
    
    create_response(200, "text/plain", objects_list.as_bytes())
}

// Utility Functions

fn create_response(status: u16, content_type: &str, body: &[u8]) -> HttpResponse {
    let headers = vec![
        ("Content-Type".to_string(), content_type.to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
    ];
    
    HttpResponse {
        status,
        headers,
        body: Some(body.to_vec()),
    }
}

fn extract_query_param(uri: &str, param: &str) -> Option<String> {
    if let Some(query_start) = uri.find('?') {
        let query = &uri[query_start + 1..];
        for pair in query.split('&') {
            if let Some(eq_pos) = pair.find('=') {
                let key = &pair[..eq_pos];
                let value = &pair[eq_pos + 1..];
                if key == param {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn format_pkt_line(line: &str) -> Vec<u8> {
    let len = line.len() + 4;
    let len_hex = format!("{:04x}", len);
    let mut result = len_hex.into_bytes();
    result.extend(line.as_bytes());
    result
}

fn format_pack_data(pack_data: &[u8]) -> Vec<u8> {
    // In Git Smart HTTP protocol, pack data is sent in packet-line format
    // Each packet can be up to 65516 bytes (65520 - 4 byte header)
    let mut result = Vec::new();
    
    // For simplicity, send the entire pack in chunks
    const MAX_PACKET_SIZE: usize = 65516;
    let mut pos = 0;
    
    while pos < pack_data.len() {
        let chunk_size = std::cmp::min(MAX_PACKET_SIZE, pack_data.len() - pos);
        let total_size = chunk_size + 4; // +4 for length header
        
        // Format as packet-line: 4-byte hex length + data
        let len_hex = format!("{:04x}", total_size);
        result.extend(len_hex.as_bytes());
        result.extend(&pack_data[pos..pos + chunk_size]);
        
        pos += chunk_size;
    }
    
    result
}

// Pack Protocol Implementation

#[derive(Debug)]
struct UploadPackRequest {
    wants: Vec<String>,
    haves: Vec<String>,
    capabilities: Vec<String>,
}

fn parse_upload_pack_request(body: &[u8]) -> UploadPackRequest {
    let mut wants = Vec::new();
    let mut haves = Vec::new();
    let mut capabilities = Vec::new();
    
    let body_str = String::from_utf8_lossy(body);
    log(&format!("Parsing upload-pack request: {} bytes", body.len()));
    
    // Parse packet-line format
    let mut pos = 0;
    while pos < body.len() {
        // Read packet length (4 hex chars)
        if pos + 4 > body.len() {
            break;
        }
        
        let len_str = String::from_utf8_lossy(&body[pos..pos + 4]);
        let packet_len = match u16::from_str_radix(&len_str, 16) {
            Ok(len) => len as usize,
            Err(_) => break,
        };
        
        if packet_len == 0 {
            // Flush packet, move to next
            pos += 4;
            continue;
        }
        
        if pos + packet_len > body.len() {
            break;
        }
        
        // Extract packet content (excluding 4-byte length prefix)
        let packet_content = String::from_utf8_lossy(&body[pos + 4..pos + packet_len]);
        let line = packet_content.trim();
        
        if line.starts_with("want ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let hash = parts[1].to_string();
                wants.push(hash);
                
                // Parse capabilities from first want line
                if parts.len() > 2 && wants.len() == 1 {
                    for cap in &parts[2..] {
                        capabilities.push(cap.to_string());
                    }
                }
            }
        } else if line.starts_with("have ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                haves.push(parts[1].to_string());
            }
        }
        
        pos += packet_len;
    }
    
    log(&format!("Parsed: {} wants, {} haves, {} capabilities", 
                 wants.len(), haves.len(), capabilities.len()));
    
    UploadPackRequest {
        wants,
        haves,
        capabilities,
    }
}



// Helper function to add an object and all its dependencies to the set
fn add_object_with_dependencies(repo_state: &GitRepoState, hash: &str, objects: &mut std::collections::HashSet<String>) {
    if objects.contains(hash) {
        return; // Already processed
    }
    
    if let Some(obj) = repo_state.objects.get(hash) {
        objects.insert(hash.to_string());
        
        match obj {
            GitObject::Commit { tree, parents, .. } => {
                // Add tree and all parent commits
                add_object_with_dependencies(repo_state, tree, objects);
                for parent in parents {
                    add_object_with_dependencies(repo_state, parent, objects);
                }
            }
            GitObject::Tree { entries } => {
                // Add all blobs and subtrees referenced by this tree
                for entry in entries {
                    add_object_with_dependencies(repo_state, &entry.hash, objects);
                }
            }
            GitObject::Tag { object, .. } => {
                // Add the object this tag points to
                add_object_with_dependencies(repo_state, object, objects);
            }
            GitObject::Blob { .. } => {
                // Blobs don't reference other objects
            }
        }
    }
}

fn generate_pack_file(repo_state: &GitRepoState, object_hashes: &[String]) -> Vec<u8> {
    log("Generating pack file");
    
    // Collect only the requested objects and their dependencies
    let mut objects_to_include = std::collections::HashSet::new();
    
    // Add requested objects and their dependencies
    for hash in object_hashes {
        add_object_with_dependencies(repo_state, hash, &mut objects_to_include);
    }
    
    // If no specific objects requested, include all objects
    if objects_to_include.is_empty() {
        for hash in repo_state.objects.keys() {
            objects_to_include.insert(hash.clone());
        }
    }
    
    let mut pack_data = Vec::new();
    
    // Pack file header: "PACK" + version (2) + number of objects
    pack_data.extend(b"PACK");
    pack_data.extend(&2u32.to_be_bytes()); // Version 2
    pack_data.extend(&(objects_to_include.len() as u32).to_be_bytes());
    
    // Add objects to pack
    for hash in &objects_to_include {
        if let Some(obj) = repo_state.objects.get(hash) {
            let (obj_type, obj_data) = match obj {
            GitObject::Blob { content } => (1u8, content.clone()), // OBJ_BLOB = 1
            GitObject::Tree { entries } => (2u8, serialize_tree_object(entries)), // OBJ_TREE = 2
            GitObject::Commit { tree, parents, author, committer, message } => {
                let commit_data = serialize_commit_object(tree, parents, author, committer, message);
                (3u8, commit_data) // OBJ_COMMIT = 3
            }
            GitObject::Tag { .. } => (4u8, vec![]), // OBJ_TAG = 4, not implemented
        };
        
        // Object header: type and size
        let size = obj_data.len();
        let mut header = vec![];
        
        // Git pack object header format
        // First byte: MTTT SSSS where M=more-size-bytes, TTT=type, SSSS=size-bits
        let mut size_to_encode = size;
        let first_byte = (obj_type << 4) | ((size_to_encode & 0x0F) as u8);
        size_to_encode >>= 4;
        
        if size_to_encode == 0 {
            // Size fits in 4 bits, no continuation needed
            header.push(first_byte);
        } else {
            // Size needs continuation bytes
            header.push(first_byte | 0x80); // Set continuation bit
            
            // Add continuation bytes
            while size_to_encode > 0 {
                let mut byte = (size_to_encode & 0x7F) as u8;
                size_to_encode >>= 7;
                if size_to_encode > 0 {
                    byte |= 0x80; // Set continuation bit if more bytes follow
                }
                header.push(byte);
            }
        }
        
        pack_data.extend(header);
        
        // Compress object data with zlib (Git requirement)
        let compressed_data = compress_zlib(&obj_data);
        pack_data.extend(compressed_data);
        
            log(&format!("Added object {} ({} bytes)", hash, size));
        }
    }
    
    // Add SHA-1 checksum of pack file content (everything before the checksum)
    let pack_checksum = calculate_pack_sha1_checksum(&pack_data);
    pack_data.extend(&pack_checksum);
    
    log(&format!("Generated pack file: {} bytes", pack_data.len()));
    pack_data
}

fn generate_empty_pack() -> Vec<u8> {
    let mut pack_data = Vec::new();
    
    // Empty pack: "PACK" + version (2) + 0 objects + checksum
    pack_data.extend(b"PACK");
    pack_data.extend(&2u32.to_be_bytes()); // Version 2
    pack_data.extend(&0u32.to_be_bytes()); // 0 objects
    
    // Add proper checksum for empty pack
    let pack_checksum = calculate_pack_sha1_checksum(&pack_data);
    pack_data.extend(&pack_checksum);
    
    pack_data
}

fn serialize_tree_object(entries: &[TreeEntry]) -> Vec<u8> {
    let mut data = Vec::new();
    
    for entry in entries {
        data.extend(entry.mode.as_bytes());
        data.push(b' ');
        data.extend(entry.name.as_bytes());
        data.push(0); // Null terminator
        
        // Convert hex hash to binary (20 bytes)
        if entry.hash.len() == 40 {
            for i in (0..40).step_by(2) {
                if let Ok(byte) = u8::from_str_radix(&entry.hash[i..i+2], 16) {
                    data.push(byte);
                }
            }
        }
    }
    
    data
}

fn serialize_commit_object(tree: &str, parents: &[String], author: &str, committer: &str, message: &str) -> Vec<u8> {
    let mut content = String::new();
    
    content.push_str(&format!("tree {}\n", tree));
    
    for parent in parents {
        content.push_str(&format!("parent {}\n", parent));
    }
    
    // For simplicity, use fixed timestamp
    let timestamp = "1609459200 +0000";
    content.push_str(&format!("author {} {}\n", author, timestamp));
    content.push_str(&format!("committer {} {}\n", committer, timestamp));
    content.push('\n');
    content.push_str(message);
    
    content.into_bytes()
}

fn calculate_git_hash(obj_type: &str, content: &[u8]) -> String {
    // Git hash = SHA-1("<type> <size>\0<content>")
    let header = format!("{} {}\0", obj_type, content.len());
    
    let mut full_content = Vec::new();
    full_content.extend(header.as_bytes());
    full_content.extend(content);
    
    let hash_bytes = sha1_hash(&full_content);
    
    // Convert to hex string
    hash_bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

fn calculate_sha1_checksum(data: &[u8]) -> Vec<u8> {
    // For simplicity, use a deterministic but fake checksum
    // A real implementation would use SHA-1
    use std::hash::{Hash, Hasher};
    
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    let hash_value = hasher.finish();
    
    // Create a 20-byte checksum that looks realistic
    let mut checksum = Vec::with_capacity(20);
    for i in 0..20 {
        checksum.push(((hash_value >> (i * 8)) & 0xFF) as u8);
    }
    checksum
}

fn compress_zlib(data: &[u8]) -> Vec<u8> {
    // Simple zlib compression implementation
    // For a basic implementation, we'll create a minimal zlib wrapper
    
    let mut compressed = Vec::new();
    
    // zlib header (RFC 1950)
    // CMF (Compression Method and flags): 0x78 (deflate, 32K window)
    // FLG (flags): 0x9C (check bits to make header checksum correct)
    compressed.extend(&[0x78, 0x9C]);
    
    // For simplicity, use "stored" (uncompressed) deflate blocks
    // This is valid deflate format but not compressed
    
    let mut pos = 0;
    while pos < data.len() {
        let chunk_size = std::cmp::min(65535, data.len() - pos);
        let is_final = pos + chunk_size >= data.len();
        
        // Block header: BFINAL (1 bit) + BTYPE (2 bits) = 00000000 or 00000001
        // BTYPE 00 = stored (uncompressed)
        compressed.push(if is_final { 0x01 } else { 0x00 });
        
        // LEN (length of data) - little endian
        compressed.extend(&(chunk_size as u16).to_le_bytes());
        
        // NLEN (one's complement of LEN) - little endian  
        compressed.extend(&(!(chunk_size as u16)).to_le_bytes());
        
        // Raw data
        compressed.extend(&data[pos..pos + chunk_size]);
        
        pos += chunk_size;
    }
    
    // Adler-32 checksum of original data
    let adler32 = calculate_adler32(data);
    compressed.extend(&adler32.to_be_bytes());
    
    compressed
}

fn calculate_adler32(data: &[u8]) -> u32 {
    // Adler-32 checksum algorithm (RFC 1950)
    const ADLER32_BASE: u32 = 65521;
    
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    
    for &byte in data {
        a = (a + byte as u32) % ADLER32_BASE;
        b = (b + a) % ADLER32_BASE;
    }
    
    (b << 16) | a
}

fn calculate_pack_sha1_checksum(pack_data: &[u8]) -> [u8; 20] {
    // Calculate SHA-1 checksum of the pack file content
    // This should be the SHA-1 of everything before the checksum itself
    sha1_hash(pack_data)
}

bindings::export!(Component with_types_in bindings);
