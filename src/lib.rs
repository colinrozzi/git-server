mod git;
mod protocol;
mod utils;

#[allow(warnings)]
mod bindings;

use bindings::exports::theater::simple::actor::Guest;
use bindings::exports::theater::simple::http_handlers::Guest as HttpHandlers;
use bindings::exports::theater::simple::http_handlers::{HandlerId, WebsocketMessage};
use bindings::theater::simple::http_framework::{self};
use bindings::theater::simple::http_types::ServerConfig;
use bindings::theater::simple::http_types::{HttpRequest, HttpResponse, MiddlewareResult};
use bindings::theater::simple::runtime::log;
use git::objects::GitObject;
use git::repository::GitRepoState;
use protocol::smart_http::{
    create_response, extract_query_param, handle_info_refs, handle_receive_pack, handle_upload_pack,
};

struct Component;

impl Guest for Component {
    fn init(state: Option<Vec<u8>>, params: (String,)) -> Result<(Option<Vec<u8>>,), String> {
        log("Initializing git-server actor");
        let (self_id,) = params;
        log(&format!("Git server actor ID: {}", &self_id));

        // Parse existing state or create new
        let mut repo_state = match state {
            Some(bytes) => serde_json::from_slice::<GitRepoState>(&bytes).unwrap_or_else(|_| {
                log("Failed to parse existing state, creating new");
                GitRepoState::default()
            }),
            None => {
                log("No existing state, creating new git repository");
                GitRepoState::default()
            }
        };

        // Ensure we have some basic objects from the start
        repo_state.ensure_minimal_objects_debug();

        log(&format!(
            "Git repository '{}' initialized with {} refs and {} objects",
            repo_state.repo_name,
            repo_state.refs.len(),
            repo_state.objects.len()
        ));

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
                log(&format!(
                    "Successfully registered git handler with ID: {}",
                    handler_id
                ));
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
            ("GET", uri) if uri.contains("/info/refs") => handle_info_refs(&repo_state, &request),
            ("POST", uri) if uri.ends_with("/git-upload-pack") => {
                handle_upload_pack(&mut repo_state, &request)
            }
            ("POST", uri) if uri.ends_with("/git-receive-pack") => {
                handle_receive_pack(&mut repo_state, &request)
            }

            // Debug/info endpoints for development
            ("GET", "/") => handle_repo_info(&repo_state),
            ("GET", "/refs") => handle_list_refs(&repo_state),
            ("GET", "/objects") => handle_list_objects(&repo_state),

            // 404 for everything else
            _ => {
                log(&format!(
                    "Unknown route: {} {}",
                    request.method, request.uri
                ));
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

bindings::export!(Component with_types_in bindings);
