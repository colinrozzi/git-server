// Fixed Protocol v2 HTTP Implementation
// Addresses the key issues found in tests

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::repository::GitRepoState;
use crate::protocol::protocol_v2_parser::ProtocolV2Parser;
use crate::utils::logging::safe_log as log;

// ============================================================================
// PROTOCOL V2 CONSTANTS - FIXED
// ============================================================================

pub const PROTOCOL_VERSION: &str = "version 2";
pub const FLUSH_PKT: &[u8] = b"0000";
pub const DELIM_PKT: &[u8] = b"0001";
pub const RESPONSE_END_PKT: &[u8] = b"0002";

// ============================================================================
// MAIN PROTOCOL V2 HANDLERS - FIXED
// ============================================================================

/// Handle GET /info/refs - Protocol v2 capability advertisement (FIXED)
pub fn handle_smart_info_refs(repo_state: &GitRepoState, service: &str) -> HttpResponse {
    log(&format!("Generating Protocol v2 capability advertisement for service: {}", service));
    
    let mut response_data = Vec::new();
    
    // Protocol v2 mandatory format - FIXED packet encoding
    response_data.extend(encode_pkt_line_fixed(b"version 2\n"));
    response_data.extend(encode_pkt_line_fixed(b"agent=git-server/0.1.0\n"));
    response_data.extend(encode_pkt_line_fixed(b"object-format=sha1\n"));
    
    match service {
        "git-receive-pack" => {
            // Push operations capabilities
            response_data.extend(encode_pkt_line_fixed(b"receive-pack=report-status delete-refs side-band-64k quiet atomic\n"));
            response_data.extend(encode_pkt_line_fixed(b"ofs-delta\n"));
        }
        "git-upload-pack" | _ => {
            // Clone/pull operations capabilities
            response_data.extend(encode_pkt_line_fixed(b"server-option\n"));
            response_data.extend(encode_pkt_line_fixed(b"ls-refs=symrefs peel ref-prefix unborn\n"));
            response_data.extend(encode_pkt_line_fixed(b"fetch=shallow thin-pack no-progress include-tag ofs-delta sideband-all wait-for-done\n"));
            response_data.extend(encode_pkt_line_fixed(b"object-info=size\n"));
        }
    }
    
    response_data.extend(FLUSH_PKT);
    
    let content_type = match service {
        "git-receive-pack" => "application/x-git-receive-pack-advertisement",
        _ => "application/x-git-upload-pack-advertisement",
    };
    
    create_response(200, content_type, &response_data)
}

/// Handle POST /git-upload-pack - Protocol v2 command processing (FIXED)
pub fn handle_upload_pack_request(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Processing Protocol v2 upload-pack request");
    
    let body = match &request.body {
        Some(b) => b,
        None => return create_error_response_fixed("missing request body"),
    };
    
    let parsed = match parse_command_request_fixed(body) {
        Ok(req) => req,
        Err(e) => return create_error_response_fixed(&e),
    };
    
    log(&format!("Parsed command: '{}'", parsed.command));
    
    match parsed.command.as_str() {
        "ls-refs" => handle_ls_refs_fixed(repo_state, &parsed),
        "fetch" => handle_fetch_fixed(repo_state, &parsed),
        "object-info" => handle_object_info_fixed(repo_state, &parsed),
        _ => create_error_response_fixed(&format!("Unknown command: {}", parsed.command)),
    }
}

/// Handle POST /git-receive-pack (FIXED)
pub fn handle_receive_pack_request(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("🎯 Processing Protocol v2 receive-pack (FIXED)");
    
    let body = match &request.body {
        Some(b) => b,
        None => return create_status_response_fixed(false, vec!["missing request body".to_string()]),
    };
    
    match ProtocolV2Parser::parse_receive_pack_request(body) {
        Ok(push) => {
            log(&format!("✅ Parsed {} ref updates, {} bytes pack", 
                        push.ref_updates.len(), push.pack_data.len()));
            
            if push.ref_updates.is_empty() && push.pack_data.is_empty() {
                return create_status_response_fixed(true, vec![]);
            }
            
            let ref_tuples = push.ref_updates
                .into_iter()
                .map(|u| (u.ref_name, u.old_oid, u.new_oid))
                .collect();
                
            match repo_state.process_push_operation(&push.pack_data, ref_tuples) {
                Ok(statuses) => {
                    let ref_statuses: Vec<String> = statuses.iter()
                        .map(|status| {
                            if status.starts_with("create ") {
                                format!("ok {}", &status[7..])
                            } else if status.starts_with("update ") {
                                format!("ok {}", &status[7..])
                            } else {
                                status.clone()
                            }
                        })
                        .collect();
                    create_status_response_fixed(true, ref_statuses)
                }
                Err(e) => create_status_response_fixed(false, vec![format!("unpack {}", e)])
            }
        }
        Err(e) => create_status_response_fixed(false, vec![format!("unpack {}", e)])
    }
}

// ============================================================================
// COMMAND HANDLERS - FIXED
// ============================================================================

#[derive(Debug)]
struct CommandRequest {
    command: String,
    capabilities: Vec<String>,
    args: Vec<String>,
}

fn handle_ls_refs_fixed(repo_state: &GitRepoState, request: &CommandRequest) -> HttpResponse {
    log("Handling ls-refs command");
    let mut response = Vec::new();
    
    if repo_state.refs.is_empty() {
        log("Empty repository - showing unborn HEAD");
        // Empty repo - show unborn HEAD
        response.extend(encode_pkt_line_fixed(b"unborn HEAD symref-target:refs/heads/main\n"));
    } else {
        let mut refs: Vec<_> = repo_state.refs.iter().collect();
        refs.sort_by_key(|(name, _)| *name);
        
        for (ref_name, hash) in refs {
            let line = format!("{} {}\n", hash, ref_name);
            response.extend(encode_pkt_line_fixed(line.as_bytes()));
        }
    }
    
    response.extend(FLUSH_PKT);
    create_response(200, "application/x-git-upload-pack-result", &response)
}

fn handle_fetch_fixed(repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response_fixed("fetch not implemented yet")
}

fn handle_object_info_fixed(repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response_fixed("object-info not implemented yet")
}

// FIXED: Command request parsing
fn parse_command_request_fixed(data: &[u8]) -> Result<CommandRequest, String> {
    let mut lines = Vec::new();
    let mut pos = 0;
    
    while pos < data.len() {
        if pos + 4 > data.len() { 
            break; 
        }
        
        let len_str = std::str::from_utf8(&data[pos..pos+4])
            .map_err(|_| "Invalid packet length header")?;
        let len = u16::from_str_radix(len_str, 16)
            .map_err(|_| format!("Invalid packet length: {}", len_str))?;
            
        if len == 0 {
            pos += 4;
            break; // Flush packet
        }
        
        if len < 4 || pos + len as usize > data.len() {
            return Err(format!("Invalid packet length: {} at position {}", len, pos));
        }
        
        let content = &data[pos+4..pos+len as usize];
        
        // FIXED: Handle strings properly, strip newlines and null terminators
        let line = std::str::from_utf8(content)
            .map_err(|_| "Invalid UTF-8 in packet")?
            .trim_end_matches('\n')
            .trim_end_matches('\0'); // Remove null terminators
        
        if !line.is_empty() {
            lines.push(line.to_string());
        }
        
        pos += len as usize;
    }
    
    if lines.is_empty() {
        return Err("No command found in request".to_string());
    }
    
    let first_line = &lines[0];
    let command = if let Some(cmd) = first_line.strip_prefix("command=") {
        cmd.to_string()
    } else {
        return Err(format!("Invalid command format: {}", first_line));
    };
    
    log(&format!("Parsed command: '{}' from '{}' lines", command, lines.len()));
    
    Ok(CommandRequest {
        command,
        capabilities: vec![],  // TODO: Parse capabilities properly
        args: lines[1..].to_vec(),
    })
}

// ============================================================================
// HTTP UTILITIES - FIXED
// ============================================================================

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

// FIXED: Error response format
fn create_error_response_fixed(message: &str) -> HttpResponse {
    let mut data = Vec::new();
    
    // Proper error packet format
    let error_line = format!("ERR {}\n", message);
    data.extend(encode_pkt_line_fixed(error_line.as_bytes()));
    data.extend(FLUSH_PKT);
    
    create_response(400, "application/x-git-upload-pack-result", &data)
}

// FIXED: Status response format
pub fn create_status_response_fixed(success: bool, ref_statuses: Vec<String>) -> HttpResponse {
    let mut data = Vec::new();
    
    // Unpack status
    if success {
        data.extend(encode_pkt_line_fixed(b"unpack ok\n"));
    } else {
        data.extend(encode_pkt_line_fixed(b"unpack failed\n"));
    }
    
    // Reference statuses
    for status in ref_statuses {
        let line = format!("{}\n", status);
        data.extend(encode_pkt_line_fixed(line.as_bytes()));
    }
    
    data.extend(FLUSH_PKT);
    create_response(200, "application/x-git-receive-pack-result", &data)
}

// FIXED: Packet line encoding - now includes the actual data!
fn encode_pkt_line_fixed(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    let mut result = format!("{:04x}", total_len).into_bytes();
    result.extend_from_slice(data);
    result
}
