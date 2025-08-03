// Git Protocol v2 HTTP Implementation
// Complete rewrite for Protocol v2 - no backwards compatibility

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::objects::GitObject;
use crate::git::repository::GitRepoState;
use crate::utils::logging::safe_log as log;
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
// MAIN PROTOCOL V2 ENTRY POINTS
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

/// Handle POST /git-receive-pack - Protocol v2 push operations
pub fn handle_receive_pack_request(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Protocol v2 push operations use fetch command with different capabilities");
    
    if request.body.is_none() {
        return create_error_response("Missing request body");
    }
    
    let body = request.body.as_ref().unwrap();
    
    // Parse the Protocol v2 command request
    let parsed_request = match parse_command_request(body) {
        Ok(req) => req,
        Err(e) => {
            log(&format!("Failed to parse v2 receive-pack request: {}", e));
            return create_error_response(&format!("Invalid request: {}", e));
        }
    };
    
    // Receive-pack uses the actual protocol flow, not command parsing
    // The body contains: ref-updates + pack-data
    let (ref_updates, pack_data) = match parse_receive_pack_data(body) {
        Ok(result) => result,
        Err(e) => {
            log(&format!("Failed to parse receive-pack data: {}", e));
            return create_status_response(false, &[format!("unpack {}", e)]);
        }
    };
    
    if ref_updates.is_empty() {
        log("No ref updates in receive-pack request");
        return create_status_response(true, &[]);
    }
    
    log(&format!("Processing {} ref updates with {} bytes of pack data", 
                ref_updates.len(), pack_data.len()));
    
    match repo_state.process_push_operation(&pack_data, ref_updates) {
        Ok(updated_refs) => {
            log(&format!("Push operation successful: {:?}", updated_refs));
            
            // Create success response with individual ref statuses
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
            log(&format!("Push operation failed: {}", e));
            create_status_response(false, &[format!("unpack {}", e)])
        }
    }
}

/// Extract service parameter from query string (legacy support for info/refs)
pub fn extract_service_from_query(query: &Option<String>) -> Option<String> {
    // In Protocol v2, service is less important, but we extract it for routing
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
        ref_prefixes.push("".to_string()); // Empty prefix matches all
    }
    
    // Handle unborn HEAD
    if show_unborn && repo_state.refs.is_empty() {
        let unborn_line = format!("unborn HEAD symref-target:{}\n", repo_state.head);
        response_data.extend(encode_pkt_line(unborn_line.as_bytes()));
    }
    
    // Generate ref listing
    let mut sorted_refs: Vec<_> = repo_state.refs.iter().collect();
    sorted_refs.sort_by_key(|(name, _)| *name);
    
    for (ref_name, hash) in sorted_refs {
        // Check if ref matches any prefix
        let matches_prefix = ref_prefixes.iter().any(|prefix| {
            prefix.is_empty() || ref_name.starts_with(prefix)
        });
        
        if !matches_prefix {
            continue;
        }
        
        let mut ref_line = format!("{} {}", hash, ref_name);
        
        // Add symref info if requested and this is HEAD
        if show_symrefs && ref_name == "HEAD" {
            ref_line.push_str(&format!(" symref-target:{}", repo_state.head));
        }
        
        // Add peeled info for annotated tags if requested
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
    
    // Flush packet to end response
    response_data.extend(encode_flush_pkt());
    
    create_response(200, "application/x-git-upload-pack-result", &response_data)
}

/// Handle fetch command - packfile transfer in Protocol v2
fn handle_fetch_command(repo_state: &GitRepoState, request: &CommandRequest) -> HttpResponse {
    log("Executing fetch command");
    
    let mut wants = HashSet::new();
    let mut haves = HashSet::new();
    let mut shallow_oids = HashSet::new();
    let mut done = false;
    let mut thin_pack = false;
    let mut no_progress = false;
    let mut include_tag = false;
    let mut ofs_delta = false;
    let mut sideband_all = false;
    let mut wait_for_done = false;
    let mut deepen: Option<u32> = None;
    let mut deepen_since = None;
    let mut deepen_not = Vec::new();
    let mut filter_spec = None;
    let mut want_refs = Vec::new();
    
    // Parse fetch arguments
    for arg in &request.args {
        if let Some(want_oid) = arg.strip_prefix("want ") {
            wants.insert(want_oid.to_string());
        } else if let Some(have_oid) = arg.strip_prefix("have ") {
            haves.insert(have_oid.to_string());
        } else if let Some(shallow_oid) = arg.strip_prefix("shallow ") {
            shallow_oids.insert(shallow_oid.to_string());
        } else if let Some(depth_str) = arg.strip_prefix("deepen ") {
            deepen = depth_str.parse().ok();
        } else if let Some(timestamp) = arg.strip_prefix("deepen-since ") {
            deepen_since = Some(timestamp.to_string());
        } else if let Some(rev) = arg.strip_prefix("deepen-not ") {
            deepen_not.push(rev.to_string());
        } else if let Some(filter) = arg.strip_prefix("filter ") {
            filter_spec = Some(filter.to_string());
        } else if let Some(ref_name) = arg.strip_prefix("want-ref ") {
            want_refs.push(ref_name.to_string());
        } else {
            match arg.as_str() {
                "done" => done = true,
                "thin-pack" => thin_pack = true,
                "no-progress" => no_progress = true,
                "include-tag" => include_tag = true,
                "ofs-delta" => ofs_delta = true,
                "sideband-all" => sideband_all = true,
                "wait-for-done" => wait_for_done = true,
                "deepen-relative" => {
                    log("Deepen-relative requested");
                }
                _ => log(&format!("Unknown fetch argument: {}", arg)),
            }
        }
    }
    
    let mut response_data = Vec::new();
    
    // Generate acknowledgments section
    if !done || !wait_for_done {
        response_data.extend(encode_pkt_line(b"acknowledgments\n"));
        
        let mut found_common = false;
        for have in &haves {
            if repo_state.objects.contains_key(have) {
                let ack_line = format!("ACK {}\n", have);
                response_data.extend(encode_pkt_line(ack_line.as_bytes()));
                found_common = true;
            }
        }
        
        if !found_common {
            response_data.extend(encode_pkt_line(b"NAK\n"));
        }
        
        if done || found_common {
            response_data.extend(encode_pkt_line(b"ready\n"));
        }
        
        response_data.extend(encode_delim_pkt());
    }
    
    // Generate shallow-info section if needed
    if !shallow_oids.is_empty() || deepen.is_some() {
        response_data.extend(encode_pkt_line(b"shallow-info\n"));
        
        // Handle shallow operations
        for shallow_oid in &shallow_oids {
            let line = format!("shallow {}\n", shallow_oid);
            response_data.extend(encode_pkt_line(line.as_bytes()));
        }
        
        response_data.extend(encode_delim_pkt());
    }
    
    // Generate wanted-refs section if want-ref was used
    if !want_refs.is_empty() {
        response_data.extend(encode_pkt_line(b"wanted-refs\n"));
        
        for want_ref in &want_refs {
            if let Some(oid) = repo_state.refs.get(want_ref) {
                let line = format!("{} {}\n", oid, want_ref);
                response_data.extend(encode_pkt_line(line.as_bytes()));
            }
        }
        
        response_data.extend(encode_delim_pkt());
    }
    
    // Generate packfile section
    if done && (!wants.is_empty() || !want_refs.is_empty()) {
        response_data.extend(encode_pkt_line(b"packfile\n"));
        
        // Collect all wanted objects
        let mut all_wants = wants.clone();
        for want_ref in &want_refs {
            if let Some(oid) = repo_state.refs.get(want_ref) {
                all_wants.insert(oid.clone());
            }
        }
        
        // Generate pack data
        match generate_pack_file(repo_state, &all_wants, &haves, thin_pack, ofs_delta) {
            Ok(pack_data) => {
                // Send pack data using sideband protocol
                let sideband_data = encode_sideband_data(&pack_data, sideband_all);
                response_data.extend(sideband_data);
            }
            Err(e) => {
                log(&format!("Failed to generate pack file: {}", e));
                let error_line = format!("ERR {}\n", e);
                response_data.extend(encode_pkt_line(error_line.as_bytes()));
            }
        }
    }
    
    // Final flush packet
    response_data.extend(encode_flush_pkt());
    
    create_response(200, "application/x-git-upload-pack-result", &response_data)
}

/// Handle object-info command - get object metadata
fn handle_object_info_command(repo_state: &GitRepoState, request: &CommandRequest) -> HttpResponse {
    log("Executing object-info command");
    
    let mut want_size = false;
    let mut object_ids = Vec::new();
    
    // Parse arguments
    for arg in &request.args {
        if arg == "size" {
            want_size = true;
        } else if let Some(oid) = arg.strip_prefix("oid ") {
            object_ids.push(oid.to_string());
        }
    }
    
    let mut response_data = Vec::new();
    
    // Send attributes header
    if want_size {
        response_data.extend(encode_pkt_line(b"size\n"));
    }
    
    // Send object info for each requested object
    for oid in &object_ids {
        if let Some(obj) = repo_state.objects.get(oid) {
            let size = calculate_object_size(obj);
            let info_line = format!("{} {}\n", oid, size);
            response_data.extend(encode_pkt_line(info_line.as_bytes()));
        }
    }
    
    response_data.extend(encode_flush_pkt());
    
    create_response(200, "application/x-git-upload-pack-result", &response_data)
}

// ============================================================================
// PROTOCOL V2 UTILITIES
// ============================================================================

/// Parse a Protocol v2 command request from raw bytes
fn parse_command_request(data: &[u8]) -> Result<CommandRequest, String> {
    let mut lines = Vec::new();
    let mut pos = 0;
    
    // Parse packet-lines
    while pos < data.len() {
        if pos + 4 > data.len() {
            break;
        }
        
        let len_str = std::str::from_utf8(&data[pos..pos + 4])
            .map_err(|_| "Invalid packet length")?;
        let len = u16::from_str_radix(len_str, 16)
            .map_err(|_| "Invalid packet length")?;
        
        if len == 0 {
            // Flush packet - end of section
            pos += 4;
            break;
        } else if len == 1 {
            // Delimiter packet
            pos += 4;
            continue;
        } else if len < 4 {
            return Err("Invalid packet length".to_string());
        }
        
        let content_len = (len - 4) as usize;
        if pos + 4 + content_len > data.len() {
            return Err("Truncated packet".to_string());
        }
        
        let content = &data[pos + 4..pos + 4 + content_len];
        let line = std::str::from_utf8(content)
            .map_err(|_| "Invalid UTF-8 in packet")?
            .trim_end_matches('\n');
        
        lines.push(line.to_string());
        pos += 4 + content_len;
    }
    
    if lines.is_empty() {
        return Err("Empty request".to_string());
    }
    
    // Parse command
    let command_line = &lines[0];
    if !command_line.starts_with("command=") {
        return Err("Missing command".to_string());
    }
    let command = command_line[8..].to_string();
    
    // Parse capabilities and arguments
    let mut capabilities = Vec::new();
    let mut args = Vec::new();
    let mut in_args = false;
    
    for line in &lines[1..] {
        if line.is_empty() {
            in_args = true;
            continue;
        }
        
        if in_args {
            args.push(line.clone());
        } else {
            // Parse capability
            if let Some(eq_pos) = line.find('=') {
                capabilities.push(Capability {
                    key: line[..eq_pos].to_string(),
                    value: Some(line[eq_pos + 1..].to_string()),
                });
            } else {
                capabilities.push(Capability {
                    key: line.clone(),
                    value: None,
                });
            }
        }
    }
    
    Ok(CommandRequest {
        command,
        capabilities,
        args,
    })
}

/// Generate pack file for Protocol v2
fn generate_pack_file(
    _repo_state: &GitRepoState,
    wants: &HashSet<String>,
    haves: &HashSet<String>,
    _thin_pack: bool,
    _ofs_delta: bool,
) -> Result<Vec<u8>, String> {
    log(&format!("Generating pack for {} wants, {} haves", wants.len(), haves.len()));
    
    // For now, return minimal pack file
    // TODO: Integrate with proper pack generation
    Ok(Vec::new())
}

/// Encode data for sideband transmission
fn encode_sideband_data(data: &[u8], _sideband_all: bool) -> Vec<u8> {
    let mut result = Vec::new();
    let chunk_size = 1000; // Leave room for sideband byte and packet header
    
    for chunk in data.chunks(chunk_size) {
        let mut packet_data = Vec::new();
        packet_data.push(1); // Sideband 1 = pack data
        packet_data.extend(chunk);
        result.extend(encode_pkt_line(&packet_data));
    }
    
    result
}

/// Calculate the size of a Git object
fn calculate_object_size(obj: &GitObject) -> usize {
    match obj {
        GitObject::Blob { content } => content.len(),
        GitObject::Tree { entries } => {
            // Calculate tree object size
            entries.iter().map(|entry| {
                format!("{} {}\0", entry.mode, entry.name).len() + 20 // +20 for SHA-1
            }).sum()
        }
        GitObject::Commit { tree, parents, author, committer, message } => {
            let mut size = format!("tree {}\n", tree).len();
            for parent in parents {
                size += format!("parent {}\n", parent).len();
            }
            size += format!("author {}\n", author).len();
            size += format!("committer {}\n", committer).len();
            size += 1; // newline before message
            size += message.len();
            size
        }
        GitObject::Tag { object, tag_type, tagger, message } => {
            let mut size = format!("object {}\n", object).len();
            size += format!("type {}\n", tag_type).len();
            size += "tag tag_name\n".len(); // Placeholder for tag name
            {
                size += format!("tagger {}\n", tagger).len();
            }
            size += 1; // newline before message
            size += message.len();
            size
        }
    }
}

// ============================================================================
// HTTP UTILITIES
// ============================================================================

/// Create an HTTP response with proper headers
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

/// Create an error response
fn create_error_response(message: &str) -> HttpResponse {
    let mut response_data = Vec::new();
    let error_line = format!("ERR {}\n", message);
    response_data.extend(encode_pkt_line(error_line.as_bytes()));
    response_data.extend(encode_flush_pkt());
    
    create_response(400, "application/x-git-upload-pack-result", &response_data)
}

/// Create receive-pack status response
pub fn create_status_response(success: bool, ref_statuses: &[String]) -> HttpResponse {
    let mut response_data = Vec::new();
    
    // Send unpack status
    let unpack_status = if success { "unpack ok" } else { "unpack error" };
    response_data.extend(encode_pkt_line(format!("{}\n", unpack_status).as_bytes()));
    
    // Send individual ref statuses
    for ref_status in ref_statuses {
        response_data.extend(encode_pkt_line(format!("{}\n", ref_status).as_bytes()));
    }
    
    // Flush packet to end response
    response_data.extend(encode_flush_pkt());
    
    create_response(200, "application/x-git-receive-pack-result", &response_data)
}

// ============================================================================
// PACKET-LINE UTILITIES
// ============================================================================

/// Encode data as a pkt-line (4-byte hex length + data)
pub fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    if total_len > 0xFFFF {
        panic!("pkt-line too long: {}", total_len);
    }
    
    let mut result = Vec::new();
    result.extend(format!("{:04x}", total_len).as_bytes());
    result.extend(data);
    result
}

/// Encode a flush packet (0000)
pub fn encode_flush_pkt() -> Vec<u8> {
    b"0000".to_vec()
}

/// Encode a delimiter packet (0001)
pub fn encode_delim_pkt() -> Vec<u8> {
    b"0001".to_vec()
}

/// Encode a response end packet (0002)
pub fn encode_response_end_pkt() -> Vec<u8> {
    b"0002".to_vec()
}

// ============================================================================
// RECEIVE-PACK SPECIFIC UTILITIES
// ============================================================================

/// Parse receive-pack data into ref-updates and pack-data components
fn parse_receive_pack_data(body: &[u8]) -> Result<(Vec<(String, String, String)>, Vec<u8>), String> {
    log("Parsing receive-pack data structure");
    
    let mut pos = 0;
    let mut ref_updates = Vec::new();
    let mut pack_data = Vec::new();
    
    // Skip any command/announcement packets
    while pos < body.len() && body[pos..].starts_with(b"000ecommand=receive-pack") {
        pos = body[pos..].windows(4).position(|w| w == b"0000").map(|p| pos + p + 4).unwrap_or(pos + 4);
    }
    
    // Parse ref updates - format: <old-sha> <new-sha> <ref-name>\0<caps>\n\n
    let lines: Vec<&[u8]> = body[pos..].split(|&b| b == b'\n').collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.is_empty() {
            i += 1;
            continue;
        }
        
        let line_str = std::str::from_utf8(line).map_err(|_| "Invalid UTF-8 in ref update")?;
        let parts: Vec<&str> = line_str.split_whitespace().collect();
        
        if parts.len() == 3 && parts[0].len() == 40 && parts[1].len() == 40 {
            let old_oid = parts[0].to_string();
            let new_oid = parts[1].to_string(); 
            let ref_name = parts[2].to_string();
            
            ref_updates.push((ref_name, old_oid, new_oid));
            i += 1;
        } else {
            // This is probably pack data
            break;
        }
    }
    
    // The rest is pack data (PACK...)
    let remaining_data = &body[pos..];
    if !remaining_data.is_empty() && remaining_data.starts_with(b"PACK") {
        pack_data = remaining_data.to_vec();
    }
    
    log(&format!("Parsed {} ref updates and {} bytes pack data", ref_updates.len(), pack_data.len()));
    Ok((ref_updates, pack_data))
}

/// Helper to validate ref format
fn is_valid_ref_name(name: &str) -> bool {
    name.starts_with("refs/heads/") || name.starts_with("refs/tags/") || name == "HEAD"}
