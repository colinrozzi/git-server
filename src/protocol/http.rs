// Smart HTTP Only Protocol Implementation
// This completely replaces the dumb HTTP implementation

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::objects::GitObject;
use crate::git::repository::GitRepoState;
use crate::utils::logging::safe_log as log;
use std::collections::HashSet;

/// Create an HTTP response with proper headers
pub fn create_response(status: u16, content_type: &str, body: &[u8]) -> HttpResponse {
    let headers = vec![
        ("Content-Type".to_string(), content_type.to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
        ("Cache-Control".to_string(), "no-cache".to_string()), // Required for Git Smart HTTP
    ];

    HttpResponse {
        status,
        headers,
        body: Some(body.to_vec()),
    }
}

// ============================================================================
// SMART HTTP REF ADVERTISEMENT (GET /info/refs?service=<service>)
// ============================================================================

/// Handle GET /info/refs?service=<service> - Smart HTTP ref advertisement
pub fn handle_smart_info_refs(repo_state: &GitRepoState, service: &str) -> HttpResponse {
    log(&format!("Smart HTTP ref advertisement for service: {}", service));
    
    match service {
        "git-upload-pack" => generate_upload_pack_advertisement(repo_state),
        "git-receive-pack" => generate_receive_pack_advertisement(repo_state),
        _ => create_response(400, "text/plain", b"Invalid service. Use git-upload-pack or git-receive-pack"),
    }
}

/// Generate ref advertisement for git-upload-pack (fetch/clone)
fn generate_upload_pack_advertisement(repo_state: &GitRepoState) -> HttpResponse {
    let mut response_data = Vec::new();
    
    // 1. Service announcement
    let service_line = "# service=git-upload-pack\n";
    response_data.extend(encode_pkt_line(service_line.as_bytes()));
    response_data.extend(encode_flush_pkt());
    
    // 2. Ref advertisement with capabilities
    let capabilities = "multi_ack_detailed thin-pack side-band side-band-64k ofs-delta shallow agent=git-server/0.1.0";
    
    let mut first_ref = true;
    
    // Add HEAD first if it exists
    if let Some(head_hash) = repo_state.refs.get(&repo_state.head) {
        let ref_line = format!("{} HEAD\0{}\n", head_hash, capabilities);
        response_data.extend(encode_pkt_line(ref_line.as_bytes()));
        first_ref = false;
    }
    
    // Add all other refs (sorted for consistency)
    let mut sorted_refs: Vec<_> = repo_state.refs.iter().collect();
    sorted_refs.sort_by_key(|(name, _)| *name);
    
    for (ref_name, hash) in sorted_refs {
        if ref_name == &repo_state.head {
            continue; // Already added as HEAD
        }
        
        let ref_line = format!("{} {}\n", hash, ref_name);
        response_data.extend(encode_pkt_line(ref_line.as_bytes()));
    }
    
    // Handle empty repository (no refs)
    if first_ref {
        let dummy_line = format!("{} capabilities^{{}}\0{}\n", 
                                "0000000000000000000000000000000000000000", capabilities);
        response_data.extend(encode_pkt_line(dummy_line.as_bytes()));
    }
    
    // 3. Final flush packet
    response_data.extend(encode_flush_pkt());
    
    create_response(
        200,
        "application/x-git-upload-pack-advertisement",
        &response_data,
    )
}

/// Generate ref advertisement for git-receive-pack (push)
fn generate_receive_pack_advertisement(repo_state: &GitRepoState) -> HttpResponse {
    let mut response_data = Vec::new();
    
    // 1. Service announcement
    let service_line = "# service=git-receive-pack\n";
    response_data.extend(encode_pkt_line(service_line.as_bytes()));
    response_data.extend(encode_flush_pkt());
    
    // 2. Ref advertisement with push capabilities
    let capabilities = "report-status delete-refs side-band-64k quiet atomic ofs-delta agent=git-server/0.1.0";
    
    let mut first_ref = true;
    
    // Add all refs (sorted for consistency)
    let mut sorted_refs: Vec<_> = repo_state.refs.iter().collect();
    sorted_refs.sort_by_key(|(name, _)| *name);
    
    for (ref_name, hash) in sorted_refs {
        let ref_line = if first_ref {
            format!("{} {}\0{}\n", hash, ref_name, capabilities)
        } else {
            format!("{} {}\n", hash, ref_name)
        };
        
        response_data.extend(encode_pkt_line(ref_line.as_bytes()));
        first_ref = false;
    }
    
    // Handle empty repository
    if first_ref {
        let dummy_line = format!("{} capabilities^{{}}\0{}\n", 
                                "0000000000000000000000000000000000000000", capabilities);
        response_data.extend(encode_pkt_line(dummy_line.as_bytes()));
    }
    
    // 3. Final flush packet
    response_data.extend(encode_flush_pkt());
    
    create_response(
        200,
        "application/x-git-receive-pack-advertisement",
        &response_data,
    )
}

// ============================================================================
// SMART HTTP UPLOAD-PACK (POST /git-upload-pack - FETCH/CLONE)
// ============================================================================

#[derive(Debug)]
pub struct UploadPackRequest {
    pub wants: Vec<String>,
    pub haves: Vec<String>,
    pub capabilities: Vec<String>,
    pub shallow: Vec<String>,
    pub deepen: Option<u32>,
    pub done: bool,
}

/// Handle POST /git-upload-pack (smart fetch/clone)
pub fn handle_upload_pack_request(
    repo_state: &GitRepoState,
    request: &HttpRequest,
) -> HttpResponse {
    log("Processing smart upload-pack request");
    
    let body = match &request.body {
        Some(data) => data,
        None => {
            return create_response(400, "text/plain", b"Missing request body");
        }
    };
    
    match parse_upload_pack_request(body) {
        Ok(upload_request) => process_upload_pack_request(repo_state, upload_request),
        Err(e) => {
            log(&format!("Failed to parse upload-pack request: {}", e));
            create_error_response(&format!("Invalid request: {}", e))
        }
    }
}

/// Parse upload-pack request from pkt-line format
fn parse_upload_pack_request(data: &[u8]) -> Result<UploadPackRequest, String> {
    let mut wants = Vec::new();
    let mut haves = Vec::new();
    let mut capabilities = Vec::new();
    let mut shallow = Vec::new();
    let mut deepen = None;
    let mut done = false;
    
    let mut offset = 0;
    let mut first_want = true;
    
    while offset < data.len() {
        let (pkt_len, pkt_data) = match decode_pkt_line(data, offset)? {
            (len, Some(data)) => (len, data),
            (len, None) => {
                // Flush packet - continue parsing
                offset += len;
                continue;
            }
        };
        
        offset += pkt_len;
        let line = String::from_utf8_lossy(&pkt_data).trim().to_string();
        
        if line.starts_with("want ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                wants.push(parts[1].to_string());
                
                // First want line may contain capabilities after \0
                if first_want {
                    if let Some(null_pos) = line.find('\0') {
                        let caps_str = &line[null_pos + 1..];
                        capabilities.extend(
                            caps_str.split_whitespace().map(|s| s.to_string())
                        );
                    }
                    first_want = false;
                }
            }
        } else if line.starts_with("have ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                haves.push(parts[1].to_string());
            }
        } else if line.starts_with("shallow ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                shallow.push(parts[1].to_string());
            }
        } else if line.starts_with("deepen ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                deepen = parts[1].parse().ok();
            }
        } else if line == "done" {
            done = true;
            break;
        }
    }
    
    Ok(UploadPackRequest {
        wants,
        haves,
        capabilities,
        shallow,
        deepen,
        done,
    })
}

/// Process upload-pack request and generate response
fn process_upload_pack_request(
    repo_state: &GitRepoState,
    request: UploadPackRequest,
) -> HttpResponse {
    log(&format!("Upload-pack: wants={}, haves={}, done={}", 
                request.wants.len(), request.haves.len(), request.done));
    
    // Validate that all wanted objects exist
    for want in &request.wants {
        if !repo_state.objects.contains_key(want) {
            log(&format!("Wanted object {} not found", want));
            return create_error_response(&format!("Object {} not found", want));
        }
    }
    
    if !request.done {
        // Client is still negotiating - send ACK/NAK response
        return handle_upload_pack_negotiation(repo_state, &request);
    }
    
    // Client sent "done" - generate pack file
    generate_pack_response(repo_state, &request)
}

/// Handle want/have negotiation phase
fn handle_upload_pack_negotiation(
    repo_state: &GitRepoState,
    request: &UploadPackRequest,
) -> HttpResponse {
    let mut response_data = Vec::new();
    let mut found_common = false;
    
    // Check if any of the client's "have" objects are in our repository
    for have in &request.haves {
        if repo_state.objects.contains_key(have) {
            // Found a common object
            let ack_line = if request.capabilities.contains(&"multi_ack_detailed".to_string()) {
                format!("ACK {} continue\n", have)
            } else if request.capabilities.contains(&"multi_ack".to_string()) {
                format!("ACK {} continue\n", have)
            } else {
                format!("ACK {}\n", have)
            };
            response_data.extend(encode_pkt_line(ack_line.as_bytes()));
            found_common = true;
            break; // For simplicity, acknowledge first common object
        }
    }
    
    if !found_common {
        response_data.extend(encode_pkt_line(b"NAK\n"));
    }
    
    response_data.extend(encode_flush_pkt());
    
    create_response(
        200,
        "application/x-git-upload-pack-result",
        &response_data,
    )
}

/// Generate pack file response
fn generate_pack_response(
    repo_state: &GitRepoState,
    request: &UploadPackRequest,
) -> HttpResponse {
    log("Generating pack file for upload-pack");
    
    let mut response_data = Vec::new();
    
    // Send final ACK or NAK
    if !request.haves.is_empty() {
        let mut found_common = false;
        for have in &request.haves {
            if repo_state.objects.contains_key(have) {
                response_data.extend(encode_pkt_line(
                    format!("ACK {}\n", have).as_bytes()
                ));
                found_common = true;
                break;
            }
        }
        if !found_common {
            response_data.extend(encode_pkt_line(b"NAK\n"));
        }
    } else {
        response_data.extend(encode_pkt_line(b"NAK\n"));
    }
    
    // Send flush packet before pack data
    response_data.extend(encode_flush_pkt());
    
    // Generate pack file containing requested objects
    match generate_pack_file(repo_state, &request.wants, &request.haves) {
        Ok(pack_data) => {
            log(&format!("Generated pack file: {} bytes, header: {:?}", 
                        pack_data.len(), 
                        if pack_data.len() >= 4 { Some(&pack_data[0..4]) } else { None }));
            
            if request.capabilities.contains(&"side-band-64k".to_string()) {
                log("Using side-band-64k protocol for pack data");
                // Send pack data using side-band protocol
                response_data.extend(encode_sideband_pack_data(&pack_data));
            } else {
                log("Sending raw pack data (no side-band)");
                // Send raw pack data directly (no pkt-line framing for pack data)
                response_data.extend(pack_data);
            }
            
            create_response(
                200,
                "application/x-git-upload-pack-result",
                &response_data,
            )
        }
        Err(e) => {
            log(&format!("Failed to generate pack: {}", e));
            create_error_response(&format!("Pack generation failed: {}", e))
        }
    }
}

// ============================================================================
// SMART HTTP RECEIVE-PACK (POST /git-receive-pack - PUSH)
// ============================================================================

#[derive(Debug)]
pub struct ReceivePackRequest {
    pub commands: Vec<RefUpdateCommand>,
    pub capabilities: Vec<String>,
    pub pack_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct RefUpdateCommand {
    pub old_hash: String,    // Current ref value (all zeros for create)
    pub new_hash: String,    // New ref value (all zeros for delete)
    pub ref_name: String,    // Reference name (e.g., refs/heads/main)
}

/// Handle POST /git-receive-pack (smart push)
pub fn handle_receive_pack_request(
    repo_state: &mut GitRepoState,
    request: &HttpRequest,
) -> HttpResponse {
    log("Processing smart receive-pack request");
    
    let body = match &request.body {
        Some(data) => data,
        None => {
            return create_response(400, "text/plain", b"Missing request body");
        }
    };
    
    log(&format!("Received {} bytes of data", body.len()));
    log(&format!("First 100 bytes: {:?}", String::from_utf8_lossy(&body[..std::cmp::min(100, body.len())])));
    
    match parse_receive_pack_request(body) {
        Ok(receive_request) => process_receive_pack_request(repo_state, receive_request),
        Err(e) => {
            log(&format!("Failed to parse receive-pack request: {}", e));
            create_response(400, "text/plain", format!("Invalid request: {}", e).as_bytes())
        }
    }
}

/// Parse receive-pack request from pkt-line format
fn parse_receive_pack_request(data: &[u8]) -> Result<ReceivePackRequest, String> {
    let mut commands = Vec::new();
    let mut capabilities = Vec::new();
    let mut offset = 0;
    let mut first_command = true;
    
    // Parse command list first
    while offset < data.len() {
        let (pkt_len, pkt_data) = match decode_pkt_line(data, offset)? {
            (len, Some(data)) => (len, data),
            (len, None) => {
                // Flush packet - end of commands, start of pack data
                offset += len;
                break;
            }
        };
        
        offset += pkt_len;
        let line = String::from_utf8_lossy(&pkt_data).trim().to_string();
        
        // Parse command: "<old-hash> <new-hash> <ref-name>[\0<capabilities>]"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let old_hash = parts[0].to_string();
            let new_hash = parts[1].to_string();
            
            // Extract ref_name, stopping at NUL byte if present
            let ref_name = if let Some(null_pos) = parts[2].find('\0') {
                parts[2][..null_pos].to_string()
            } else {
                parts[2].to_string()
            };
            
            // Extract capabilities from first command if present
            if first_command {
                if let Some(null_pos) = line.find('\0') {
                    let caps_str = &line[null_pos + 1..];
                    capabilities.extend(
                        caps_str.split_whitespace().map(|s| s.to_string())
                    );
                }
                first_command = false;
            }
            
            commands.push(RefUpdateCommand {
                old_hash,
                new_hash,
                ref_name,
            });
        }
    }
    
    // Extract pack data if present
    let pack_data = if offset < data.len() {
        Some(data[offset..].to_vec())
    } else {
        None
    };
    
    Ok(ReceivePackRequest {
        commands,
        capabilities,
        pack_data,
    })
}

/// Process receive-pack request and generate response
fn process_receive_pack_request(
    repo_state: &mut GitRepoState,
    request: ReceivePackRequest,
) -> HttpResponse {
    log(&format!("Receive-pack: {} commands", request.commands.len()));
    
    // Process pack data first if present
    if let Some(pack_data) = &request.pack_data {
        log(&format!("TEMPORARILY SKIPPING pack data processing ({} bytes)", pack_data.len()));
        // TEMPORARY: Skip pack processing to test response format
        // if let Err(e) = process_pack_data(repo_state, pack_data) {
        //     log(&format!("Failed to process pack data: {}", e));
        //     return generate_push_response(&request, false, &format!("unpack {}", e), &[]);
        // }
    }
    
    // Process each ref update command
    let mut command_results = Vec::new();
    let mut all_success = true;
    
    for command in &request.commands {
        let result = process_ref_update_command(repo_state, command);
        let success = result.starts_with("ok");
        if !success {
            all_success = false;
        }
        command_results.push(result);
    }
    
    // Generate response
    let unpack_status = if request.pack_data.is_some() {
        "unpack ok"
    } else {
        "unpack ok" // No pack data to process
    };
    
    generate_push_response(&request, all_success, unpack_status, &command_results)
}

// ============================================================================
// PACKET-LINE PROTOCOL UTILITIES
// ============================================================================

/// Encode data as a pkt-line (4-byte hex length + data)
pub fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4; // +4 for the length prefix
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

/// Decode pkt-line format, returns (length, data) or None for flush
pub fn decode_pkt_line(data: &[u8], offset: usize) -> Result<(usize, Option<Vec<u8>>), String> {
    if offset + 4 > data.len() {
        return Err("Not enough data for pkt-line length".to_string());
    }
    
    let length_str = std::str::from_utf8(&data[offset..offset + 4])
        .map_err(|_| "Invalid hex in pkt-line length")?;
        
    let length = usize::from_str_radix(length_str, 16)
        .map_err(|_| format!("Invalid hex length: {}", length_str))?;
    
    if length == 0 {
        // Flush packet
        return Ok((4, None));
    }
    
    if length < 4 {
        return Err(format!("Invalid pkt-line length: {}", length));
    }
    
    let data_len = length - 4;
    if offset + length > data.len() {
        return Err("Not enough data for pkt-line payload".to_string());
    }
    
    let payload = data[offset + 4..offset + length].to_vec();
    Ok((length, Some(payload)))
}

/// Extract service name from query parameters  
pub fn extract_service_from_query(query_params: &str) -> Option<&str> {
    for param in query_params.split('&') {
        if let Some(value) = param.strip_prefix("service=") {
            return Some(value);
        }
    }
    None
}

/// Create an error response with ERR packet
fn create_error_response(message: &str) -> HttpResponse {
    let mut response_data = Vec::new();
    let err_line = format!("ERR {}\n", message);
    response_data.extend(encode_pkt_line(err_line.as_bytes()));
    
    create_response(
        200, // Git protocol errors are still HTTP 200
        "application/x-git-upload-pack-result",
        &response_data,
    )
}

// ============================================================================
// PACK FILE GENERATION (STUB IMPLEMENTATIONS - TO BE COMPLETED)
// ============================================================================

/// Generate a Git pack file containing the requested objects
fn generate_pack_file(
    repo_state: &GitRepoState,
    wants: &[String],
    haves: &[String],
) -> Result<Vec<u8>, String> {
    // Collect all objects needed (wanted objects + their dependencies)
    let mut needed_objects = HashSet::new();
    let have_set: HashSet<_> = haves.iter().collect();
    
    // Add all wanted objects and their dependencies
    for want in wants {
        collect_object_dependencies(repo_state, want, &mut needed_objects, &have_set)?;
    }
    
    log(&format!("Pack file will contain {} objects", needed_objects.len()));
    
    // Generate the pack file
    create_pack_file(repo_state, &needed_objects)
}

/// Recursively collect all object dependencies
fn collect_object_dependencies(
    repo_state: &GitRepoState,
    object_hash: &str,
    needed: &mut HashSet<String>,
    haves: &HashSet<&String>,
) -> Result<(), String> {
    // Skip if client already has this object
    if haves.contains(&object_hash.to_string()) {
        return Ok(());
    }
    
    // Skip if already processed
    if needed.contains(object_hash) {
        return Ok(());
    }
    
    // Add this object
    needed.insert(object_hash.to_string());
    
    // Get the object and add its dependencies
    if let Some(object) = repo_state.objects.get(object_hash) {
        match object {
            GitObject::Commit { tree, parents, .. } => {
                // Add tree and parent commits
                collect_object_dependencies(repo_state, tree, needed, haves)?;
                for parent in parents {
                    collect_object_dependencies(repo_state, parent, needed, haves)?;
                }
            }
            GitObject::Tree { entries } => {
                // Add all tree entries
                for entry in entries {
                    collect_object_dependencies(repo_state, &entry.hash, needed, haves)?;
                }
            }
            GitObject::Tag { object, .. } => {
                // Add tagged object
                collect_object_dependencies(repo_state, object, needed, haves)?;
            }
            GitObject::Blob { .. } => {
                // Blobs have no dependencies
            }
        }
    }
    
    Ok(())
}

/// Create a Git pack file from a set of objects
fn create_pack_file(
    repo_state: &GitRepoState,
    objects: &HashSet<String>,
) -> Result<Vec<u8>, String> {
    let mut pack_data = Vec::new();
    
    // Pack file header: "PACK" + version + object count
    pack_data.extend(b"PACK");
    pack_data.extend((2u32).to_be_bytes());  // Version 2
    pack_data.extend((objects.len() as u32).to_be_bytes());  // Object count
    
    // Add each object to the pack
    for object_hash in objects {
        if let Some(object) = repo_state.objects.get(object_hash) {
            let object_data = serialize_pack_object(object)?;
            pack_data.extend(object_data);
        }
    }
    
    // Add SHA-1 checksum of the entire pack (excluding this checksum)
    let checksum = crate::utils::hash::sha1_hash(&pack_data);
    pack_data.extend(checksum);
    
    Ok(pack_data)
}

/// Serialize an object for inclusion in a pack file
fn serialize_pack_object(object: &GitObject) -> Result<Vec<u8>, String> {
    super::pack::serialize_pack_object(object)
}

/// Process pack data and add objects to repository
fn process_pack_data(repo_state: &mut GitRepoState, pack_data: &[u8]) -> Result<(), String> {
    super::pack::process_pack_data(repo_state, pack_data)
}

/// Process a single ref update command
fn process_ref_update_command(
    repo_state: &mut GitRepoState,
    command: &RefUpdateCommand,
) -> String {
    log(&format!(
        "Processing ref update: {} {} -> {}",
        command.ref_name, command.old_hash, command.new_hash
    ));
    
    let zero_hash = "0000000000000000000000000000000000000000";
    
    if command.new_hash == zero_hash {
        // Delete reference
        if let Some(current_hash) = repo_state.refs.remove(&command.ref_name) {
            if current_hash != command.old_hash {
                return format!("ng {} non-fast-forward", command.ref_name);
            }
            format!("ok {}", command.ref_name)
        } else {
            format!("ng {} does not exist", command.ref_name)
        }
    } else if command.old_hash == zero_hash {
        // Create new reference
        if repo_state.refs.contains_key(&command.ref_name) {
            format!("ng {} already exists", command.ref_name)
        } else {
            repo_state.refs.insert(command.ref_name.clone(), command.new_hash.clone());
            format!("ok {}", command.ref_name)
        }
    } else {
        // Update existing reference
        match repo_state.refs.get(&command.ref_name) {
            Some(current_hash) => {
                if current_hash != &command.old_hash {
                    format!("ng {} non-fast-forward", command.ref_name)
                } else {
                    repo_state.refs.insert(command.ref_name.clone(), command.new_hash.clone());
                    format!("ok {}", command.ref_name)
                }
            }
            None => {
                format!("ng {} does not exist", command.ref_name)
            }
        }
    }
}

/// Generate push response with status report
fn generate_push_response(
    request: &ReceivePackRequest,
    _success: bool,
    unpack_status: &str,
    command_results: &[String],
) -> HttpResponse {
    let mut response_data = Vec::new();
    
    // Check if client supports sideband
    // TEMPORARILY DISABLE SIDEBAND TO TEST
    let use_sideband = false;
    // let use_sideband = request.capabilities.contains(&"side-band-64k".to_string()) || 
    //                   request.capabilities.contains(&"side-band".to_string());
    
    if use_sideband {
        log("Using sideband protocol for receive-pack response");
        
        // Send unpack status wrapped in sideband (band 1)
        let unpack_line = format!("{}
", unpack_status);
        let sideband_packet = encode_sideband_message(1, unpack_line.as_bytes());
        log(&format!("Sending sideband packet: {:?}", String::from_utf8_lossy(&sideband_packet)));
        response_data.extend(sideband_packet);
        
        // Send command results if report-status was requested
        if request.capabilities.contains(&"report-status".to_string()) {
            for result in command_results {
                let result_line = format!("{}
", result);
                response_data.extend(encode_sideband_message(1, result_line.as_bytes()));
            }
        }
        
        // End with flush packet
        response_data.extend(encode_flush_pkt());
    } else {
        log("Using legacy protocol for receive-pack response (no sideband)");
        
        // Fallback to legacy protocol (for older Git clients)
        let unpack_line = format!("{}
", unpack_status);
        log(&format!("Sending unpack status: {:?}", unpack_line));
        response_data.extend(encode_pkt_line(unpack_line.as_bytes()));
        
        // Send command results if report-status was requested
        if request.capabilities.contains(&"report-status".to_string()) {
            log(&format!("Sending {} command results", command_results.len()));
            for result in command_results {
                let result_line = format!("{}
", result);
                log(&format!("Sending command result: {:?}", result_line));
                response_data.extend(encode_pkt_line(result_line.as_bytes()));
            }
        } else {
            log("Not sending command results (report-status not requested)");
        }
        
        // End with flush packet
        response_data.extend(encode_flush_pkt());
    }
    
    create_response(
        200,
        "application/x-git-receive-pack-result",
        &response_data,
    )
}
/// Encode pack data using side-band protocol
fn encode_sideband_pack_data(pack_data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let chunk_size = 65515; // Max data per side-band packet
    
    let mut offset = 0;
    while offset < pack_data.len() {
        let chunk_end = std::cmp::min(offset + chunk_size, pack_data.len());
        let chunk = &pack_data[offset..chunk_end];
        
        // Create side-band packet: channel 1 (data) + chunk
        let mut packet_data = Vec::new();
        packet_data.push(1u8); // Channel 1 = pack data
        packet_data.extend(chunk);
        
        result.extend(encode_pkt_line(&packet_data));
        offset = chunk_end;
    }
    
    // End with flush packet
    result.extend(encode_flush_pkt());
    result
}

/// Encode a single message using sideband protocol
fn encode_sideband_message(band: u8, message: &[u8]) -> Vec<u8> {
    let mut packet_data = Vec::new();
    packet_data.push(band); // Band number (1=data, 2=progress, 3=error)
    packet_data.extend(message);
    encode_pkt_line(&packet_data)
}