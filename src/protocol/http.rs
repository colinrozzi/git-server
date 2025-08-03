// Git Protocol v2 HTTP Implementation - CORRECTED
// Complete rewrite for Protocol v2 working push support

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::objects::GitObject;
use crate::git::repository::GitRepoState;
use crate::utils::logging::safe_log as log;
use crate::protocol::protocol_v2_parser::{ProtocolV2Parser, PushRequest};
use std::collections::HashSet;

// ============================================================================
// PROTOCOL V2 CONSTANTS AND TYPES
// ============================================================================

pub const PROTOCOL_VERSION: &str = "version 2";

// Special packet types in v2
pub const FLUSH_PKT: &[u8] = b"0000";
pub const DELIM_PKT: &[u8] = b"0001";  
pub const RESPONSE_END_PKT: &[u8] = b"0002";

#[derive(Debug, Clone)]
pub struct Capability {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug)]
pub struct CommandRequest {
    pub command: String,
    pub capabilities: Vec<Capability>,
    pub args: Vec<String>,
}

// ============================================================================
// MAIN PROTOCOL V2 ENTRY POINTS - FIXED
// ============================================================================

/// Handle GET /info/refs - Protocol v2 capability advertisement
pub fn handle_smart_info_refs(repo_state: &GitRepoState, service: &str) -> HttpResponse {
    log(&format!("Generating Protocol v2 capability advertisement for service: {}", service));
    
    let mut response_data = Vec::new();
    
    // 1. Protocol version announcement
    response_data.extend(encode_pkt_line(b"version 2\n"));
    
    // 2. Agent capability
    response_data.extend(encode_pkt_line(b"agent=git-server/0.1.0\n"));
    
    // 3. Object format capability  
    response_data.extend(encode_pkt_line(b"object-format=sha1\n"));
    
    match service {
        "git-receive-pack" => {
            // 4. Receive-pack specific capabilities for push operations
            response_data.extend(encode_pkt_line(b"receive-pack=report-status delete-refs side-band-64k quiet atomic\n"));
            response_data.extend(encode_pkt_line(b"ofs-delta\n"));
        }
        "git-upload-pack" | _ => {
            // 4. Upload-pack capabilities for clone/pull operations
            response_data.extend(encode_pkt_line(b"server-option\n"));
            response_data.extend(encode_pkt_line(b"ls-refs=symrefs peel ref-prefix unborn\n"));
            response_data.extend(encode_pkt_line(b"fetch=shallow thin-pack no-progress include-tag ofs-delta sideband-all wait-for-done\n"));
            response_data.extend(encode_pkt_line(b"object-info=size\n"));
        }
    }
    
    // 5. Flush packet to end advertisement
    response_data.extend(encode_flush_pkt());
    
    let content_type = match service {
        "git-receive-pack" => "application/x-git-receive-pack-advertisement",
        _ => "application/x-git-upload-pack-advertisement",
    };
    
    create_response(200, content_type, &response_data)
}

/// Handle POST /git-upload-pack - Protocol v2 command processing
pub fn handle_upload_pack_request(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Processing Protocol v2 upload-pack request");
    
    // Parse the request body to extract command and arguments
    let default_body = Vec::new();
    let body = request.body.as_ref().unwrap_or(&default_body);
    let parsed_request = match parse_command_request(body) {
        Ok(req) => req,
        Err(e) => {
            log(&format!("Failed to parse v2 request: {}", e));
            return create_error_response(&format!("Invalid request: {}", e));
        }
    };
    
    log(&format!("Executing command: {}", parsed_request.command));
    
    // Route to appropriate command handler
    match parsed_request.command.as_str() {
        "ls-refs" => handle_ls_refs_command(repo_state, &parsed_request),
        "fetch" => handle_fetch_command(repo_state, &parsed_request),
        "object-info" => handle_object_info_command(repo_state, &parsed_request),
        _ => create_error_response(&format!("Unknown command: {}", parsed_request.command)),
    }
}

/// CORRECTED: Handle POST /git-receive-pack - Protocol v2 push operations
pub fn handle_receive_pack_request(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("🔍 Processing Protocol v2 receive-pack request (CORRECTED VERSION)");
    
    if request.body.is_none() {
        log("❌ Missing request body");
        return create_status_response(false, &["unpack missing-request"]);
    }
    
    let body = request.body.as_ref().unwrap();
    
    // Parse using NEW Protocol v2 binary parser
    match crate::protocol::protocol_v2_parser::ProtocolV2Parser::parse_receive_pack_request(body) {
        Ok(push_request) => {
            log(&format!("✅ Parsed Protocol v2 push: {} ref updates, {} bytes pack", 
                        push_request.ref_updates.len(), push_request.pack_data.len()));
            
            let ref_count = push_request.ref_updates.len();
            let pack_size = push_request.pack_data.len();
            
            if ref_count == 0 && pack_size == 0 {
                log("ℹ️  No-op push detected");
                return create_status_response(true, &[]);
            }
            
            // Handle empty repository special case
            if repo_state.refs.is_empty() && !push_request.ref_updates.is_empty() {
                log("🎨 Empty repository - accepting first push");
            }
            
            // Extract ref updates tuples
            let ref_updates: Vec<(String, String, String)> = push_request.ref_updates
                .into_iter()
                .map(|update| (update.ref_name, update.old_oid, update.new_oid))
                .collect();
            
            log("🔄 Processing push operation...");
            
            match repo_state.process_push_operation(&push_request.pack_data, ref_updates) {
                Ok(updated_refs) => {
                    log(&format!("✅ Push operation successful! Updated refs: {:?}", updated_refs));
                    
                    // Generate Git-compatible response
                    let ref_statuses: Vec<String> = updated_refs.iter()
                        .map(|status| {
                            if status.starts_with("create ") {
                                let ref_name = &status[7..];
                                format!("ok {}", ref_name)
                            } else if status.starts_with("update ") {
                                let ref_name = &status[7..];
                                format!("ok {}", ref_name)
                            } else {
                                status.clone()
                            }
                        })
                        .collect();
                    
                    create_status_response(true, &ref_statuses)
                }
                Err(e) => {
                    let error_msg = format!("❌ Push operation failed: {}", e);
                    log(&error_msg);
                    create_status_response(false, &[&format!("unpack {}", e)])
                }
            }
        }
        Err(parse_error) => {
            log(&format!("❌ Protocol v2 parse error: {}", parse_error));
            create_status_response(false, &[&format!("unpack {}", parse_error)])
        }
    }
}

// DEPRECATED: Mark old broken parser
#[deprecated(note = "Use ProtocolV2Parser::parse_receive_pack_request instead")]
fn parse_receive_pack_data(_body: &[u8]) -> Result<(Vec<(String, String, String)>, Vec<u8>), String> {
    Err("Protocol v2 parser replaced".to_string())
}

/// Extract service parameter from query string
pub fn extract_service_from_query(query: &Option<String>) -> Option<String> {
    query.as_ref().and_then(|q| {
        q.split('&')
            .find(|param| param.starts_with("service="))
            .map(|param| param[8..].to_string())
    })
}

// ============================================================================
// COMMAND IMPLEMENTATIONS
// ============================================================================

/// Handle ls-refs command - reference listing in Protocol v2
fn handle_ls_refs_command(repo_state: &GitRepoState, request: &CommandRequest) -> HttpResponse {
    log("Executing ls-refs command");
    
    let mut response_data = Vec::new();
    let mut show_symrefs = false;
    let mut show_peeled = false;
    let mut show_unborn = false;
    let mut ref_prefixes = Vec::new();
    
    // Parse arguments
    for arg in &request.args {
        match arg.as_str() {
            "symrefs" => show_symrefs = true,
            "peel" => show_peeled = true,
            "unborn" => show_unborn = true,
            s if s.starts_with("ref-prefix ") => {
                ref_prefixes.push(s[11..].to_string());
            }
            _ => log(&format!("Unknown ls-refs argument: {}", arg)),
        }
    }
    
    // If no prefixes specified, show all refs
    if ref_prefixes.is_empty() {
        ref_prefixes.push("".to_string());
    }
    
    // Handle unborn HEAD in empty repository
    if show_unborn && repo_state.refs.is_empty() {
        let unborn_line = format!("unborn HEAD symref-target:{}\n", repo_state.head);
        response_data.extend(encode_pkt_line(unborn_line.as_bytes()));
    }
    
    // Generate ref listing
    let mut sorted_refs: Vec<_> = repo_state.refs.iter().collect();
    sorted_refs.sort_by_key(|(name, _)| *name);
    
    for (ref_name, hash) in sorted_refs {
        let matches_prefix = ref_prefixes.iter().any(|prefix| {
            prefix.is_empty() || ref_name.starts_with(prefix)
        });
        
        if !matches_prefix {
            continue;
        }
        
        let mut ref_line = format!("{} {}", hash, ref_name);
        
        if show_symrefs && ref_name == "HEAD" {
            ref_line.push_str(&format!(" symref-target:{}", repo_state.head));
        }
        
        if show_peeled && ref_name.starts_with("refs/tags/") {
            if let Some(obj) = repo_state.objects.get(hash) {
                if let GitObject::Tag { object, .. } = obj {
                    ref_line.push_str(&format!(" peeled:{}", object));
                }
            }
        }
        
        ref_line.push('\n');
        response_data.extend(encode_pkt_line(ref_line.as_bytes()));
    }
    
    response_data.extend(encode_flush_pkt());
    create_response(200, "application/x-git-upload-pack-result", &response_data)
}

// ============================================================================
// HTTP UTILITIES
// ============================================================================

/// Create HTTP response with proper headers
pub fn create_response(status: u16, content_type: &str, body: &[u8]) -> HttpResponse {
    let headers = vec![
        ("Content-Type".to_string(), content_type.to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
        ("Cache-Control".to_string(), "no-cache".to_string()),
    ];
    
    HttpResponse {
        status,
        headers,
        body: Some(body.to_vec()),
    }
}

/// Create error response
fn create_error_response(message: &str) -> HttpResponse {
    let mut response_data = Vec::new();
    let error_line = format!("ERR {}\n", message);
    response_data.extend(encode_pkt_line(error_line.as_bytes()));
    response_data.extend(encode_flush_pkt());
    create_response(400, "application/x-git-upload-pack-result", &response_data)
}

/// Create receive-pack status response (CORRECTED)
pub fn create_status_response(success: bool, ref_statuses: &[str]) -> HttpResponse {
    let mut response_data = Vec::new();
    
    // Git protocol status format
    let unpack_status = if success { b"unpack ok\n" } else { b"unpack error\n" };
    response_data.extend_from_slice(b"0009"); // 9 chars including newline
    response_data.extend_from_slice(unpack_status);
    
    // Individual ref statuses
    for status in ref_statuses {
        let line = format!("{}\n", status);
        response_data.extend(encode_pkt_line(line.as_bytes()));
    }
    
    // End of response
    response_data.extend(encode_flush_pkt());
    create_response(200, "application/x-git-receive-pack-result", &response_data)
}

/// Packet-line utilities
pub fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    if total_len > 0xFFFF {
        return format!("{:04x}", 0xFFFF).as_bytes().to_vec();
    }
    
    let mut result = Vec::new();
    result.extend(format!("{:04x}", total_len).as_bytes());
    result.extend(data);
    result
}

pub fn encode_flush_pkt() -> Vec<u8> { b"0000".to_vec() }
pub fn encode_delim_pkt() -> Vec<u8> { b"0001".to_vec() }