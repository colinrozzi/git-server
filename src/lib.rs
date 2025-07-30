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
        let repo_state = match state {
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

        // Register a git handler
        let git_handler = http_framework::register_handler("git")
            .map_err(|e| format!("Failed to register git handler: {}", e))?;

        // Add git protocol routes
        let routes = vec![
            // Git Smart HTTP Protocol endpoints
            ("/info/refs", "GET", git_handler),
            ("/git-upload-pack", "POST", git_handler),
            ("/git-receive-pack", "POST", git_handler),
            // Debug endpoints
            ("/", "GET", git_handler),
            ("/refs", "GET", git_handler),
            ("/objects", "GET", git_handler),
        ];

        for (path, method, handler) in routes {
            http_framework::add_route(server_id, path, method, handler)
                .map_err(|e| format!("Failed to add {} {} route: {}", method, path, e))?;
            log(&format!("Added {} {} route", method, path));
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

fn handle_upload_pack(_repo_state: &mut GitRepoState, _request: &HttpRequest) -> HttpResponse {
    log("Handling upload-pack request (clone/fetch data transfer)");
    
    // For now, return a minimal response
    // TODO: Parse want/have negotiation from request body
    // TODO: Generate pack file with requested objects
    
    let response_body = b"0000"; // Empty pack for now
    
    create_response(
        200,
        "application/x-git-upload-pack-result",
        response_body
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

bindings::export!(Component with_types_in bindings);
