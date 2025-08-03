// Git Protocol v2 HTTP Implementation - CORRECTED
// Clean implementation addressing "protocol v2 not implemented yet" error

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::repository::GitRepoState;
use crate::protocol::protocol_v2_parser::ProtocolV2Parser;
use crate::utils::logging::safe_log as log;
use std::collections::HashSet;

// ============================================================================
// PROTOCOL V2 CONSTANTS
// ============================================================================

pub const PROTOCOL_VERSION: &str = "version 2";
pub const FLUSH_PKT: &[u8] = b"0000";
pub const DELIM_PKT: &[u8] = b"0001";
pub const RESPONSE_END_PKT: &[u8] = b"0002";

// ============================================================================
// MAIN PROTOCOL V2 HANDLERS - FIXED
// ============================================================================

/// Handle GET /info/refs - Protocol v2 capability advertisement
pub fn handle_smart_info_refs(repo_state: &GitRepoState, service: &str) -> HttpResponse {
    log(&format!("Generating Protocol v2 capability advertisement for service: {}", service));
    
    let mut response_data = Vec::new();
    
    // Protocol v2 mandatory format
    response_data.extend(encode_pkt_line(b"version 2\n"));
    response_data.extend(encode_pkt_line(b"agent=git-server/0.1.0\n"));
    response_data.extend(encode_pkt_line(b"object-format=sha1\n"));
    
    match service {
        "git-receive-pack" => {
            // Push operations capabilities
            response_data.extend(encode_pkt_line(b"receive-pack=report-status delete-refs side-band-64k quiet atomic\n"));
            response_data.extend(encode_pkt_line(b"ofs-delta\n"));
        }
        "git-upload-pack" | _ => {
            // Clone/pull operations capabilities
            response_data.extend(encode_pkt_line(b"server-option\n"));
            response_data.extend(encode_pkt_line(b"ls-refs=symrefs peel ref-prefix unborn\n"));
            response_data.extend(encode_pkt_line(b"fetch=shallow thin-pack no-progress include-tag ofs-delta sideband-all wait-for-done\n"));
            response_data.extend(encode_pkt_line(b"object-info=size\n"));
        }
    }
    
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
    
    let body = match &request.body {
        Some(b) => b,
        None => return create_status_response(false, vec!["unpack missing-request".to_string()]),
    };
    
    let parsed = match parse_command_request(body) {
        Ok(req) => req,
        Err(e) => return create_error_response(&e),
    };
    
    match parsed.command.as_str() {
        "ls-refs" => handle_ls_refs(repo_state, &parsed),
        "fetch" => handle_fetch(repo_state, &parsed),
        "object-info" => handle_object_info(repo_state, &parsed),
        _ => create_error_response(&format!("Unknown command: {}", parsed.command)),
    }
}

/// CORRECTED: Handle POST /git-receive-pack using new Protocol v2 binary parser
pub fn handle_receive_pack_request(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("🎯 Processing Protocol v2 receive-pack (CORRECTED)");
    
    let body = match &request.body {
        Some(b) => b,
        None => return create_status_response(false, vec!["unpack missing-request".to_string()]),
    };
    
    match ProtocolV2Parser::parse_receive_pack_request(body) {
        Ok(push) => {
            log(&format!("✅ Parsed {} ref updates, {} bytes pack", 
                        push.ref_updates.len(), push.pack_data.len()));
            
            if push.ref_updates.is_empty() && push.pack_data.is_empty() {
                return create_status_response(true, vec![]);
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
                    create_status_response(true, ref_statuses)
                }
                Err(e) => create_status_response(false, vec![format!("unpack {}", e)])
            }
        }
        Err(e) => create_status_response(false, vec![format!("unpack {}", e)])
    }
}

// ============================================================================
// COMMAND HANDLERS
// ============================================================================

#[derive(Debug)]
struct CommandRequest {
    command: String,
    capabilities: Vec<String>,
    args: Vec<String>,
}

fn handle_ls_refs(repo_state: &GitRepoState, request: &CommandRequest) -> HttpResponse {
    log("Handling ls-refs command");
    let mut response = Vec::new();
    
    if repo_state.refs.is_empty() {
        log("Empty repository - showing unborn HEAD");
        // Empty repo - show unborn HEAD
        response.extend(encode_pkt_line("unborn HEAD symref-target:refs/heads/main\n".as_bytes()));
    } else {
        let mut refs: Vec<_> = repo_state.refs.iter().collect();
        refs.sort_by_key(|(name, _)| *name);
        
        for (ref_name, hash) in refs {
            let line = format!("{} {}\n", hash, ref_name);
            response.extend(encode_pkt_line(line.as_bytes()));
        }
    }
    
    response.extend(encode_flush_pkt());
    create_response(200, "application/x-git-upload-pack-result", &response)
}

fn handle_fetch(repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response("fetch not implemented yet")
}

fn handle_object_info(repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response("object-info not implemented yet")
}

fn parse_command_request(data: &[u8]) -> Result<CommandRequest, String> {
    log(&format!("Parsing command request, data length: {} bytes", data.len()));
    log(&format!("Raw data hex: {}", hex::encode(data)));
    
    let mut lines = Vec::new();
    let mut pos = 0;
    
    while pos < data.len() {
        if pos + 4 > data.len() { break; }
        
        let len_str = std::str::from_utf8(&data[pos..pos+4])
            .map_err(|_| "Invalid packet")?;
        let len = u16::from_str_radix(len_str, 16)
            .map_err(|_| "Invalid packet length")?;
            
        if len == 0 {
            pos += 4;
            break;
        }
        
        if len < 4 || pos + len as usize > data.len() {
            return Err("Invalid packet".to_string());
        }
        
        let content = &data[pos+4..pos+len as usize];
        log(&format!("Packet {}: len={}, content_bytes={:?}", lines.len(), len, content));
        
        let line = std::str::from_utf8(content)
            .map_err(|_| "Invalid UTF-8")?
            .trim_end_matches('\n'); // Only remove trailing newline as per protocol
        
        log(&format!("Parsed line: '{}'", line));
        
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
// HTTP UTILITIES
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

fn create_error_response(message: &str) -> HttpResponse {
    let mut data = Vec::new();
    
    // Proper error packet format
    let error_line = format!("ERR {}\n", message);
    data.extend(encode_pkt_line(error_line.as_bytes()));
    data.extend(encode_flush_pkt());
    
    create_response(400, "application/x-git-upload-pack-result", &data)
}

pub fn create_status_response(success: bool, ref_statuses: Vec<String>) -> HttpResponse {
    let mut data = Vec::new();
    
    // Unpack status
    if success {
        data.extend(encode_pkt_line(b"unpack ok\n"));
    } else {
        data.extend(encode_pkt_line(b"unpack failed\n"));
    }
    
    // Reference statuses
    for status in ref_statuses {
        let line = format!("{}\n", status);
        data.extend(encode_pkt_line(line.as_bytes()));
    }
    
    data.extend(encode_flush_pkt());
    create_response(200, "application/x-git-receive-pack-result", &data)
}

// FIXED: Packet utilities
fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    let mut result = format!("{:04x}", total_len).into_bytes();
    result.extend_from_slice(data);
    result
}

fn encode_flush_pkt() -> Vec<u8> { b"0000".to_vec() }