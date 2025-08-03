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
use protocol::http::{
    create_response, handle_receive_pack_request, handle_smart_info_refs,
    handle_upload_pack_request,
};

struct Component;

impl Guest for Component {
    fn init(state: Option<Vec<u8>>, params: (String,)) -> Result<(Option<Vec<u8>>,), String> {
        log("🚀 Initializing git-server actor with Protocol v2!");
        let (self_id,) = params;
        log(&format!("Git server actor ID: {}", &self_id));

        // Parse existing state or create new
        let repo_state = match state {
            Some(bytes) => serde_json::from_slice::<GitRepoState>(&bytes).unwrap_or_else(|_| {
                log("Failed to parse existing state, creating new");
                GitRepoState::default()
            }),
            None => {
                log("No existing state, creating new git repository");
                GitRepoState::default()
            }
        };

        // Start with empty repository - modern push-first workflow
        log("🏗️  Starting with empty repository for modern push-first workflow");

        log(&format!(
            "📦 Git repository '{}' initialized with {} refs and {} objects",
            repo_state.repo_name,
            repo_state.refs.len(),
            repo_state.objects.len()
        ));

        // Set up HTTP server for Git Protocol v2
        let config = ServerConfig {
            port: Some(8080),
            host: Some("0.0.0.0".to_string()),
            tls_config: None,
        };

        // Create the server
        let server_id = http_framework::create_server(&config)
            .map_err(|e| format!("Failed to create HTTP server: {}", e))?;

        // Register git handler
        let git_handler = match http_framework::register_handler("git") {
            Ok(handler_id) => {
                log(&format!(
                    "✅ Successfully registered git handler with ID: {}",
                    handler_id
                ));
                handler_id
            }
            Err(e) => {
                log(&format!("❌ Failed to register git handler: {}", e));
                return Err(format!("Failed to register git handler: {}", e));
            }
        };

        // Add Protocol v2 routes
        match http_framework::add_route(server_id, "/info/refs", "GET", git_handler) {
            Ok(_) => log("✅ Added GET /info/refs route (Protocol v2)"),
            Err(e) => {
                log(&format!("❌ Failed to add /info/refs route: {}", e));
                return Err(format!("Failed to add /info/refs route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/git-upload-pack", "POST", git_handler) {
            Ok(_) => log("✅ Added POST /git-upload-pack route (Protocol v2)"),
            Err(e) => {
                log(&format!("❌ Failed to add /git-upload-pack route: {}", e));
                return Err(format!("Failed to add /git-upload-pack route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/git-receive-pack", "POST", git_handler) {
            Ok(_) => log("✅ Added POST /git-receive-pack route (Protocol v2)"),
            Err(e) => {
                log(&format!("❌ Failed to add /git-receive-pack route: {}", e));
                return Err(format!("Failed to add /git-receive-pack route: {}", e));
            }
        }

        // Add modern debug routes
        match http_framework::add_route(server_id, "/", "GET", git_handler) {
            Ok(_) => log("✅ Added GET / debug route"),
            Err(e) => {
                log(&format!("❌ Failed to add / route: {}", e));
                return Err(format!("Failed to add / route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/refs", "GET", git_handler) {
            Ok(_) => log("✅ Added GET /refs debug route"),
            Err(e) => {
                log(&format!("❌ Failed to add /refs route: {}", e));
                return Err(format!("Failed to add /refs route: {}", e));
            }
        }

        match http_framework::add_route(server_id, "/objects", "GET", git_handler) {
            Ok(_) => log("✅ Added GET /objects debug route"),
            Err(e) => {
                log(&format!("❌ Failed to add /objects route: {}", e));
                return Err(format!("Failed to add /objects route: {}", e));
            }
        }

        // Start the server
        match http_framework::start_server(server_id) {
            Ok(_) => log("🌐 HTTP server started successfully on port 8080"),
            Err(e) => {
                log(&format!("❌ Failed to start HTTP server: {}", e));
                return Err(format!("Failed to start HTTP server: {}", e));
            }
        }

        log("🎉 Git Protocol v2 server initialization completed!");

        // Serialize and return the repository state
        let serialized_state = serde_json::to_vec(&repo_state)
            .map_err(|e| format!("Failed to serialize state: {}", e))?;

        Ok((Some(serialized_state),))
    }
}

impl HttpHandlers for Component {
    fn handle_request(
        state: Option<Vec<u8>>,
        params: (HandlerId, HttpRequest),
    ) -> Result<(Option<Vec<u8>>, (HttpResponse,)), String> {
        let (handler_id, request) = params;
        log(&format!(
            "request received: {} {} (handler: {})",
            request.method, request.uri, handler_id
        ));

        // Parse current state
        let mut repo_state = match state {
            Some(bytes) => serde_json::from_slice::<GitRepoState>(&bytes).unwrap_or_default(),
            None => GitRepoState::default(),
        };

        // Route based on path - Protocol v2 only!
        // Parse the URI to get path and query
        let uri_parts: Vec<&str> = request.uri.splitn(2, '?').collect();
        let path = uri_parts[0];
        let query = if uri_parts.len() > 1 {
            Some(uri_parts[1].to_string())
        } else {
            None
        };

        let response = match path {
            "/info/refs" => {
                log("processing /info/refs request");
                if let Some(service) = query.as_ref().and_then(|q| {
                    q.split('&')
                        .find(|p| p.starts_with("service="))
                        .map(|p| p[8..].to_string())
                }) {
                    handle_smart_info_refs(&repo_state, &service)
                } else {
                    create_response(400, "text/plain", b"Missing service parameter")
                }
            }
            "/git-upload-pack" => {
                log("processing upload-pack");
                handle_upload_pack_request(&mut repo_state, &request)
            }
            "/git-receive-pack" => {
                log("processing receive-pack");
                handle_receive_pack_request(&mut repo_state, &request)
            }
            "/" => {
                // Modern debug endpoint
                let info = format!(
                    "🚀 Git Server Actor - Protocol v2 ONLY\n\n\
                     🏗️  Modern Git Wire Protocol Implementation\n\
                     Repository: {}\n\
                     Refs: {}\n\
                     Objects: {}\n\
                     HEAD: {}\n\n\
                     🌟 Protocol v2 Features:\n\
                     ✅ Capability advertisement\n\
                     ✅ ls-refs command\n\
                     ✅ fetch command\n\
                     ✅ object-info command\n\
                     ✅ Structured responses\n\
                     ✅ Sideband multiplexing\n\
                     ✅ Modern packet-line protocol\n\n\
                     🔗 Endpoints:\n\
                     GET  /info/refs?service=git-upload-pack\n\
                     POST /git-upload-pack\n\
                     POST /git-receive-pack\n\
                     GET  /refs (debug)\n\
                     GET  /objects (debug)\n\n\
                     💡 Usage:\n\
                     git clone http://localhost:8080\n\
                     git -c protocol.version=2 clone http://localhost:8080\n",
                    repo_state.repo_name,
                    repo_state.refs.len(),
                    repo_state.objects.len(),
                    repo_state.head
                );
                create_response(200, "text/plain", info.as_bytes())
            }
            "/refs" => {
                // Modern refs debug endpoint
                let mut refs_info = String::new();
                refs_info.push_str("🔗 Git References (Protocol v2)\n\n");

                if repo_state.refs.is_empty() {
                    refs_info.push_str("📭 No refs found (empty repository)\n\n");
                    refs_info.push_str("💡 To add refs:\n");
                    refs_info.push_str("   git push http://localhost:8080 main\n");
                } else {
                    for (ref_name, hash) in &repo_state.refs {
                        refs_info.push_str(&format!("📎 {} -> {}\n", ref_name, hash));
                    }
                }

                create_response(200, "text/plain", refs_info.as_bytes())
            }
            "/objects" => {
                // Modern objects debug endpoint
                let mut objects_info = String::new();
                objects_info.push_str("📦 Git Objects (Protocol v2)\n\n");

                if repo_state.objects.is_empty() {
                    objects_info.push_str("📭 No objects found (empty repository)\n\n");
                    objects_info.push_str("💡 Objects will be created when you push commits\n");
                } else {
                    for (hash, obj) in &repo_state.objects {
                        let (obj_type, emoji) = match obj {
                            GitObject::Blob { .. } => ("blob", "📄"),
                            GitObject::Tree { .. } => ("tree", "📁"),
                            GitObject::Commit { .. } => ("commit", "💾"),
                            GitObject::Tag { .. } => ("tag", "🏷️"),
                        };
                        objects_info.push_str(&format!("{} {} ({})\n", emoji, hash, obj_type));
                    }
                }

                create_response(200, "text/plain", objects_info.as_bytes())
            }
            _ => {
                log(&format!("❓ Unknown path: {}", path));
                create_response(404, "text/plain", "Not Found\n\nProtocol v2 endpoints:\n- GET /info/refs\n- POST /git-upload-pack\n- POST /git-receive-pack".as_bytes())
            }
        };

        log(&format!("Response: {}", response.status));

        match response.body {
            Some(ref body) => log(&format!("Response body: {}", String::from_utf8_lossy(body))),
            None => log("No response body"),
        }

        // Serialize updated state
        let serialized_state = serde_json::to_vec(&repo_state)
            .map_err(|e| format!("Failed to serialize state: {}", e))?;

        Ok((Some(serialized_state), (response,)))
    }

    fn handle_middleware(
        state: Option<Vec<u8>>,
        _params: (HandlerId, HttpRequest),
    ) -> Result<(Option<Vec<u8>>, (MiddlewareResult,)), String> {
        Ok((
            state,
            (MiddlewareResult {
                proceed: true,
                request: _params.1,
            },),
        ))
    }

    fn handle_websocket_connect(
        state: Option<Vec<u8>>,
        _params: (HandlerId, u64, String, Option<String>),
    ) -> Result<(Option<Vec<u8>>,), String> {
        Ok((state,))
    }

    fn handle_websocket_disconnect(
        state: Option<Vec<u8>>,
        _params: (HandlerId, u64),
    ) -> Result<(Option<Vec<u8>>,), String> {
        Ok((state,))
    }

    fn handle_websocket_message(
        _state: Option<Vec<u8>>,
        _params: (HandlerId, u64, WebsocketMessage),
    ) -> Result<(Option<Vec<u8>>, (Vec<WebsocketMessage>,)), String> {
        // WebSocket not used for Git protocol
        Err("WebSocket not supported for Git Protocol v2".to_string())
    }
}

bindings::export!(Component with_types_in bindings);
