// Git Protocol Handler - Supporting both v1 and v2
// FIXED: Handle Protocol v1 fallback for push operations

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::repository::GitRepoState;
use crate::protocol::protocol_v2_parser::ProtocolV2Parser;
use crate::utils::logging::safe_log as log;

// ============================================================================
// PROTOCOL CONSTANTS
// ============================================================================

pub const PROTOCOL_VERSION: &str = "version 2";
pub const FLUSH_PKT: &[u8] = b"0000";
pub const DELIM_PKT: &[u8] = b"0001";
pub const RESPONSE_END_PKT: &[u8] = b"0002";

// ============================================================================
// MAIN PROTOCOL HANDLERS - FIXED FOR DUAL PROTOCOL SUPPORT
// ============================================================================

/// Handle GET /info/refs - Support both Protocol v1 and v2
pub fn handle_smart_info_refs(repo_state: &GitRepoState, service: &str) -> HttpResponse {
    log(&format!(
        "Processing info/refs request for service: {}",
        service
    ));

    match service {
        "git-upload-pack" => {
            // Upload-pack supports Protocol v2
            handle_upload_pack_info_refs()
        }
        "git-receive-pack" => {
            // Receive-pack falls back to Protocol v1 for compatibility
            handle_receive_pack_info_refs_v1(repo_state)
        }
        _ => create_error_response("Unknown service"),
    }
}

/// Protocol v2 capability advertisement for upload-pack (fetch operations)
fn handle_upload_pack_info_refs() -> HttpResponse {
    log("Generating Protocol v2 capability advertisement for upload-pack");

    let mut response_data = Vec::new();

    // Protocol v2 format for upload-pack
    response_data.extend(encode_pkt_line(b"version 2\n"));
    response_data.extend(encode_pkt_line(b"agent=git-server/0.1.0\n"));
    response_data.extend(encode_pkt_line(b"object-format=sha1\n"));
    response_data.extend(encode_pkt_line(b"server-option\n"));
    response_data.extend(encode_pkt_line(b"ls-refs=symrefs peel ref-prefix unborn\n"));
    response_data.extend(encode_pkt_line(
        b"fetch=shallow thin-pack no-progress include-tag ofs-delta sideband-all wait-for-done\n",
    ));
    response_data.extend(encode_pkt_line(b"object-info=size\n"));
    response_data.extend(encode_flush_pkt());

    create_response(
        200,
        "application/x-git-upload-pack-advertisement",
        &response_data,
    )
}

/// Protocol v1 capability advertisement for receive-pack (push operations)
fn handle_receive_pack_info_refs_v1(repo_state: &GitRepoState) -> HttpResponse {
    log("Generating Protocol v1 capability advertisement for receive-pack (push compatibility)");

    let mut response_data = Vec::new();

    //
    // 1. Smart-HTTP banner
    //
    let banner = b"# service=git-receive-pack\n";
    response_data.extend(encode_pkt_line(banner));
    response_data.extend(encode_flush_pkt()); // flush-pkt after banner

    // Protocol v1 format - advertise refs first, then capabilities
    if repo_state.refs.is_empty() {
        // Empty repository - advertise capabilities on the null ref
        let capabilities =
            "report-status delete-refs side-band-64k quiet atomic ofs-delta agent=git-server/0.1.0";
        let line = format!(
            "0000000000000000000000000000000000000000 capabilities^{{}}\0{}\n",
            capabilities
        );
        response_data.extend(encode_pkt_line(line.as_bytes()));
    } else {
        // Advertise existing refs with capabilities on the first ref
        let mut refs: Vec<_> = repo_state.refs.iter().collect();
        refs.sort_by_key(|(name, _)| *name);

        let mut first_ref = true;
        for (ref_name, hash) in refs {
            if first_ref {
                // First ref includes capabilities
                let capabilities = "report-status delete-refs side-band-64k quiet atomic ofs-delta agent=git-server/0.1.0";
                let line = format!("{} {}\0{}\n", hash, ref_name, capabilities);
                response_data.extend(encode_pkt_line(line.as_bytes()));
                first_ref = false;
            } else {
                let line = format!("{} {}\n", hash, ref_name);
                response_data.extend(encode_pkt_line(line.as_bytes()));
            }
        }
    }

    response_data.extend(encode_flush_pkt());

    log("returning response");
    log(&String::from_utf8(response_data.clone()).unwrap());
    create_response(
        200,
        "application/x-git-receive-pack-advertisement",
        &response_data,
    )
}

/// Handle POST /git-upload-pack - Protocol v2 command processing
pub fn handle_upload_pack_request(
    repo_state: &mut GitRepoState,
    request: &HttpRequest,
) -> HttpResponse {
    log("Processing Protocol v2 upload-pack request");

    let body = match &request.body {
        Some(b) => b,
        None => return create_error_response("Missing request body"),
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

/// Handle POST /git-receive-pack - Protocol v1 push processing
pub fn handle_receive_pack_request(
    repo_state: &mut GitRepoState,
    request: &HttpRequest,
) -> HttpResponse {
    log("processing receive-pack request");

    let body = match &request.body {
        Some(b) => b,
        None => {
            log("missing request body, returning with a status response");
            return create_status_response(false, vec!["unpack missing-request".to_string()]);
        }
    };

    log("body found, parsing request");
    match parse_v1_receive_pack_request(body) {
        Ok(push) => handle_v1_push(repo_state, push),
        Err(e) => create_status_response(false, vec![format!("unpack {}", e)]),
    }
}

// ============================================================================
// PROTOCOL V1 PUSH PARSING (for compatibility)
// ============================================================================

#[derive(Debug)]
struct V1PushRequest {
    ref_updates: Vec<(String, String, String)>, // (ref_name, old_oid, new_oid)
    pack_data: Vec<u8>,
}

fn parse_v1_receive_pack_request(data: &[u8]) -> Result<V1PushRequest, String> {
    log("Parsing Protocol v1 receive-pack request");

    let mut cursor = 0;
    let mut ref_updates = Vec::new();
    let mut pack_start = 0;

    // Phase 1: Parse ref update commands
    while cursor < data.len() {
        if cursor + 4 > data.len() {
            break;
        }

        // Check for PACK signature
        if data[cursor..].starts_with(b"PACK") {
            pack_start = cursor;
            break;
        }

        let len_str =
            std::str::from_utf8(&data[cursor..cursor + 4]).map_err(|_| "Invalid packet")?;
        let len = u16::from_str_radix(len_str, 16).map_err(|_| "Invalid packet length")?;

        if len == 0 {
            cursor += 4;
            continue; // Flush packet
        }

        if len < 4 || cursor + len as usize > data.len() {
            return Err("Invalid packet".to_string());
        }

        let content = &data[cursor + 4..cursor + len as usize];
        let line = std::str::from_utf8(content)
            .map_err(|_| "Invalid UTF-8")?
            .trim_end_matches('\n');

        // Parse ref update line: "old-oid new-oid ref-name"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            ref_updates.push((
                parts[2].to_string(), // ref name
                parts[0].to_string(), // old oid
                parts[1].to_string(), // new oid
            ));
            log(&format!(
                "Parsed ref update: {} {} -> {}",
                parts[2], parts[0], parts[1]
            ));
        }

        cursor += len as usize;
    }

    // Phase 2: Extract pack data
    let pack_data = if pack_start > 0 {
        data[pack_start..].to_vec()
    } else {
        Vec::new()
    };

    log(&format!(
        "Parsed {} ref updates, {} bytes pack data",
        ref_updates.len(),
        pack_data.len()
    ));

    Ok(V1PushRequest {
        ref_updates,
        pack_data,
    })
}

fn handle_v1_push(repo_state: &mut GitRepoState, push: V1PushRequest) -> HttpResponse {
    log("Processing Protocol v1 push operation");

    if push.ref_updates.is_empty() && push.pack_data.is_empty() {
        return create_status_response(true, vec![]);
    }

    match repo_state.process_push_operation(&push.pack_data, push.ref_updates) {
        Ok(statuses) => {
            log("Push operation successful, processing statuses");
            let ref_statuses: Vec<String> = statuses
                .iter()
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
        Err(e) => create_status_response(false, vec![format!("unpack {}", e)]),
    }
}

fn handle_v2_push(
    repo_state: &mut GitRepoState,
    push: crate::protocol::protocol_v2_parser::PushRequest,
) -> HttpResponse {
    log("Processing Protocol v2 push operation");

    if push.ref_updates.is_empty() && push.pack_data.is_empty() {
        return create_status_response(true, vec![]);
    }

    let ref_tuples = push
        .ref_updates
        .into_iter()
        .map(|u| (u.ref_name, u.old_oid, u.new_oid))
        .collect();

    match repo_state.process_push_operation(&push.pack_data, ref_tuples) {
        Ok(statuses) => {
            let ref_statuses: Vec<String> = statuses
                .iter()
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
        Err(e) => create_status_response(false, vec![format!("unpack {}", e)]),
    }
}

// ============================================================================
// PROTOCOL V2 COMMAND HANDLERS
// ============================================================================

#[derive(Debug)]
struct CommandRequest {
    command: String,
    capabilities: Vec<String>,
    args: Vec<String>,
}

fn handle_ls_refs(repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    log("Handling ls-refs command");
    let mut response = Vec::new();

    if repo_state.refs.is_empty() {
        log("Empty repository - showing unborn HEAD");
        response.extend(encode_pkt_line(
            "unborn HEAD symref-target:refs/heads/main\n".as_bytes(),
        ));
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

fn handle_fetch(_repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response("fetch not implemented yet")
}

fn handle_object_info(_repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response("object-info not implemented yet")
}

fn parse_command_request(data: &[u8]) -> Result<CommandRequest, String> {
    log(&format!(
        "Parsing command request, data length: {} bytes",
        data.len()
    ));

    let mut lines = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if pos + 4 > data.len() {
            break;
        }

        let len_str = std::str::from_utf8(&data[pos..pos + 4]).map_err(|_| "Invalid packet")?;
        let len = u16::from_str_radix(len_str, 16).map_err(|_| "Invalid packet length")?;

        if len == 0 {
            pos += 4;
            break;
        }

        if len < 4 || pos + len as usize > data.len() {
            return Err("Invalid packet".to_string());
        }

        let content = &data[pos + 4..pos + len as usize];
        let line = std::str::from_utf8(content)
            .map_err(|_| "Invalid UTF-8")?
            .trim_end_matches('\n');

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

    log(&format!("Parsed command: '{}'", command));

    Ok(CommandRequest {
        command,
        capabilities: vec![],
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

// Packet utilities
fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    let mut result = format!("{:04x}", total_len).into_bytes();
    result.extend_from_slice(data);
    result
}

fn encode_flush_pkt() -> Vec<u8> {
    b"0000".to_vec()
}
