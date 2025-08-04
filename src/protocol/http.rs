// Git Protocol Handler - Supporting both v1 and v2
// FIXED: Handle Protocol v1 fallback for push operations

use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::repository::GitRepoState;
use crate::utils::logging::safe_log as log;

const CAPABILITIES: &str = "report-status delete-refs ofs-delta agent=git-server/0.1.0";
const MAX_PKT_PAYLOAD: usize = 0xFFF0 - 4; // pkt-line payload limit = 65 516
const MAX_SIDEBAND_DATA: usize = MAX_PKT_PAYLOAD - 1; // minus 1-byte channel

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
        b"fetch=shallow thin-pack no-progress include-tag ofs-delta wait-for-done\n",
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
        let line = format!(
            "0000000000000000000000000000000000000000 capabilities^{{}}\0{}\n",
            CAPABILITIES
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
                let line = format!("{} {}\0{}\n", hash, ref_name, CAPABILITIES);
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
    log("handle_receive_pack");

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
        Err(e) => {
            // For parse errors, we don't have capabilities yet, so use basic response
            create_status_response(false, vec![format!("unpack {}", e)])
        }
    }
}

// ============================================================================
// PROTOCOL V1 PUSH PARSING (for compatibility)
// ============================================================================

#[derive(Debug)]
struct V1PushRequest {
    ref_updates: Vec<(String, String, String)>, // (ref_name, old_oid, new_oid)
    pack_data: Vec<u8>,
    capabilities: Vec<String>, // Client-requested capabilities
}

fn parse_v1_receive_pack_request(data: &[u8]) -> Result<V1PushRequest, String> {
    log("Parsing Protocol v1 receive-pack request");

    let mut cursor = 0;
    let mut ref_updates = Vec::new();
    let mut capabilities = Vec::new();
    let mut pack_start = 0;
    let mut first_ref = true;

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

        // Parse ref update line: "old-oid new-oid ref-name [\0capabilities]"
        if first_ref {
            // First ref may contain capabilities after null byte
            if let Some(null_pos) = line.find('\0') {
                let ref_part = &line[..null_pos];
                let cap_part = &line[null_pos + 1..];

                // Parse capabilities
                capabilities = cap_part.split_whitespace().map(|s| s.to_string()).collect();

                log(&format!("Parsed capabilities: {:?}", capabilities));

                // Parse ref update from the part before null
                let parts: Vec<&str> = ref_part.split_whitespace().collect();
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
            } else {
                // No capabilities, parse as normal
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
            }
            first_ref = false;
        } else {
            // Subsequent refs don't have capabilities
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
        capabilities,
    })
}

fn handle_v1_push(repo_state: &mut GitRepoState, push: V1PushRequest) -> HttpResponse {
    log("Processing Protocol v1 push operation");

    if push.ref_updates.is_empty() && push.pack_data.is_empty() {
        return create_status_response_with_capabilities(true, vec![], &push.capabilities);
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
            create_status_response_with_capabilities(true, ref_statuses, &push.capabilities)
        }
        Err(e) => create_status_response_with_capabilities(
            false,
            vec![format!("unpack {}", e)],
            &push.capabilities,
        ),
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

fn handle_fetch(repo_state: &GitRepoState, request: &CommandRequest) -> HttpResponse {
    log("Handling Protocol v2 fetch command");

    // Parse want lines from request args
    let mut wants = Vec::new();
    let mut has_done = false;

    for arg in &request.args {
        if arg.starts_with("want ") {
            wants.push(arg[5..].to_string()); // Remove "want " prefix
            log(&format!("Client wants: {}", &arg[5..]));
        } else if arg == "done" {
            has_done = true;
            log("Client sent 'done' - negotiation finished, skipping acknowledgments");
        }
    }

    if wants.is_empty() {
        log("Error: No wants specified in fetch request");
        return create_error_response("No wants specified");
    }

    log(&format!(
        "Fetch request: wants={}, done={}",
        wants.len(),
        has_done
    ));

    // Generate packfile for wanted objects
    match generate_packfile_for_wants(repo_state, &wants) {
        Ok(packfile) => {
            log(&format!("Generated packfile: {} bytes", packfile.len()));

            let mut response = Vec::new();

            /* ----- 1.  acknowledgments  (only when !has_done) ----- */
            if !has_done {
                response.extend(encode_pkt_line(b"acknowledgments\n"));
                response.extend(encode_pkt_line(b"NAK\n")); // or real ACK/ready lines
                response.extend(b"0001"); // delim-pkt -> next section
            }

            // Packfile section header
            response.extend(encode_pkt_line(b"packfile\n"));

            /* ----- 3.  side-band-encode the pack ----- */
            let mut pos = 0;
            while pos < packfile.len() {
                let chunk_size = std::cmp::min(MAX_SIDEBAND_DATA, packfile.len() - pos); // 65 515
                let chunk = &packfile[pos..pos + chunk_size];
                // Pre-pend ASCII '1' (0x31) – channel 1 = data
                let mut sideband = Vec::with_capacity(1 + chunk.len());
                sideband.push(b'1');
                sideband.extend_from_slice(chunk);
                response.extend(encode_pkt_line(&sideband));
                pos += chunk_size;
            }

            // End packfile section with flush packet
            response.extend(encode_flush_pkt()); // 0000 - end of response

            log(&format!("Total response size: {} bytes", response.len()));
            create_response(200, "application/x-git-upload-pack-result", &response)
        }
        Err(e) => {
            log(&format!("Failed to generate packfile: {}", e));
            create_error_response(&format!("packfile generation failed: {}", e))
        }
    }
}

fn handle_object_info(_repo_state: &GitRepoState, _request: &CommandRequest) -> HttpResponse {
    create_error_response("object-info not implemented yet")
}

fn parse_command_request(data: &[u8]) -> Result<CommandRequest, String> {
    log(&format!(
        "Parsing Protocol v2 command request, data length: {} bytes",
        data.len()
    ));

    let mut lines = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if pos + 4 > data.len() {
            break;
        }

        let len_bytes = &data[pos..pos + 4];
        let len_str = std::str::from_utf8(len_bytes).map_err(|_| "Invalid packet")?;
        let len = u16::from_str_radix(len_str, 16).map_err(|_| "Invalid packet length")?;

        if len == 0 {
            // Flush packet - end of request
            pos += 4;
            break;
        }

        if len == 1 {
            // Delimiter packet - continue
            pos += 4;
            continue;
        }

        if len < 4 {
            return Err(format!("Invalid packet length: {} (must be >= 4)", len));
        }

        if pos + len as usize > data.len() {
            return Err(format!(
                "Packet extends beyond data: need {} bytes, have {}",
                len,
                data.len() - pos
            ));
        }

        let content = &data[pos + 4..pos + len as usize];
        let line = std::str::from_utf8(content)
            .map_err(|e| format!("Invalid UTF-8 in packet content: {}", e))?
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

    log(&format!(
        "Parsed Protocol v2 command: '{}' with {} args",
        command,
        lines.len() - 1
    ));

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
    create_status_response_with_capabilities(success, ref_statuses, &[])
}

pub fn create_status_response_with_capabilities(
    success: bool,
    ref_statuses: Vec<String>,
    capabilities: &[String],
) -> HttpResponse {
    let mut data = Vec::new();
    let use_sideband = capabilities.contains(&"side-band-64k".to_string());

    log(&format!(
        "Creating status response with sideband: {}",
        use_sideband
    ));

    // Unpack status
    if success {
        if use_sideband {
            data.extend(encode_status_message(b"unpack ok\n"));
        } else {
            data.extend(encode_pkt_line(b"unpack ok\n"));
        }
    } else {
        if use_sideband {
            data.extend(encode_status_message(b"unpack failed\n"));
        } else {
            data.extend(encode_pkt_line(b"unpack failed\n"));
        }
    }

    // Reference statuses
    for status in ref_statuses {
        let line = format!("{}\n", status);
        if use_sideband {
            data.extend(encode_status_message(line.as_bytes()));
        } else {
            data.extend(encode_pkt_line(line.as_bytes()));
        }
    }

    data.extend(encode_flush_pkt());
    create_response(200, "application/x-git-receive-pack-result", &data)
}

// ============================================================================
// PACKFILE GENERATION FOR FETCH
// ============================================================================

fn generate_packfile_for_wants(
    repo_state: &GitRepoState,
    wants: &[String],
) -> Result<Vec<u8>, String> {
    log(&format!("Generating packfile for {} wants", wants.len()));

    // Collect all objects needed for the wants
    let objects_to_send = collect_objects_for_wants(repo_state, wants)?;
    log(&format!(
        "Collected {} objects to send",
        objects_to_send.len()
    ));

    // Generate the packfile
    generate_simple_packfile(repo_state, &objects_to_send)
}

fn collect_objects_for_wants(
    repo_state: &GitRepoState,
    wants: &[String],
) -> Result<Vec<String>, String> {
    use std::collections::HashSet;
    let mut objects = HashSet::new();

    for want_hash in wants {
        // Add the wanted object itself
        objects.insert(want_hash.clone());

        // If it's a commit, traverse to get tree + blobs
        if let Some(obj) = repo_state.objects.get(want_hash) {
            match obj {
                crate::git::objects::GitObject::Commit { tree, parents, .. } => {
                    // Add the tree
                    objects.insert(tree.clone());

                    // Add all objects in the tree
                    collect_tree_objects(repo_state, tree, &mut objects)?;

                    // Add parent commits recursively
                    for parent_hash in parents {
                        collect_commit_ancestors(repo_state, parent_hash, &mut objects)?;
                    }
                }
                crate::git::objects::GitObject::Tree { .. } => {
                    // If want is a tree, collect all its objects
                    collect_tree_objects(repo_state, want_hash, &mut objects)?;
                }
                _ => {
                    // Blob or tag, just include it
                }
            }
        } else {
            return Err(format!("Wanted object not found: {}", want_hash));
        }
    }

    Ok(objects.into_iter().collect())
}

fn collect_tree_objects(
    repo_state: &GitRepoState,
    tree_hash: &str,
    objects: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    if let Some(crate::git::objects::GitObject::Tree { entries }) =
        repo_state.objects.get(tree_hash)
    {
        for entry in entries {
            objects.insert(entry.hash.clone());

            // If this entry is also a tree, recurse
            if entry.mode == "040000" {
                // Directory mode
                collect_tree_objects(repo_state, &entry.hash, objects)?;
            }
        }
    }
    Ok(())
}

fn collect_commit_ancestors(
    repo_state: &GitRepoState,
    commit_hash: &str,
    objects: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    if objects.contains(commit_hash) {
        return Ok(()); // Already processed
    }

    objects.insert(commit_hash.to_string());

    if let Some(crate::git::objects::GitObject::Commit { tree, parents, .. }) =
        repo_state.objects.get(commit_hash)
    {
        // Add the tree and its contents
        objects.insert(tree.clone());
        collect_tree_objects(repo_state, tree, objects)?;

        // Recurse to parents
        for parent_hash in parents {
            collect_commit_ancestors(repo_state, parent_hash, objects)?;
        }
    }

    Ok(())
}

fn generate_simple_packfile(
    repo_state: &GitRepoState,
    object_ids: &[String],
) -> Result<Vec<u8>, String> {
    use crate::utils::hash::sha1_hash;

    let mut pack = Vec::new();

    // Pack header: "PACK" + version(2) + object_count
    pack.extend(b"PACK");
    pack.extend(&2u32.to_be_bytes()); // version 2
    pack.extend(&(object_ids.len() as u32).to_be_bytes());

    log(&format!(
        "Pack header: version=2, objects={}",
        object_ids.len()
    ));

    // Add each object
    for obj_id in object_ids {
        if let Some(obj) = repo_state.objects.get(obj_id) {
            let obj_data = serialize_object_for_pack(obj)?;
            pack.extend(&obj_data);
        } else {
            return Err(format!("Object not found: {}", obj_id));
        }
    }

    // Pack checksum (SHA1 of entire pack so far)
    let checksum = sha1_hash(&pack);
    pack.extend(&checksum);

    log(&format!("Generated packfile: {} bytes", pack.len()));
    Ok(pack)
}

fn serialize_object_for_pack(obj: &crate::git::objects::GitObject) -> Result<Vec<u8>, String> {
    use crate::utils::compression::compress_zlib;

    let (obj_type, obj_data) = match obj {
        crate::git::objects::GitObject::Blob { content } => {
            (1u8, content.clone()) // OBJ_BLOB = 1
        }
        crate::git::objects::GitObject::Tree { entries } => {
            (2u8, serialize_tree_entries(entries)?) // OBJ_TREE = 2
        }
        crate::git::objects::GitObject::Commit {
            tree,
            parents,
            author,
            committer,
            message,
        } => {
            (
                3u8,
                serialize_commit_data(tree, parents, author, committer, message)?,
            ) // OBJ_COMMIT = 3
        }
        crate::git::objects::GitObject::Tag {
            object,
            tag_type,
            tagger,
            message,
        } => {
            (4u8, serialize_tag_data(object, tag_type, tagger, message)?) // OBJ_TAG = 4
        }
    };

    let mut result = Vec::new();

    // Encode object header (type + size)
    encode_pack_object_header(&mut result, obj_type, obj_data.len());

    // Compress and add object data
    let compressed_data = compress_zlib(&obj_data);
    result.extend(&compressed_data);

    Ok(result)
}

fn encode_pack_object_header(output: &mut Vec<u8>, obj_type: u8, size: usize) {
    let mut size = size;
    let mut byte = (obj_type << 4) | (size & 0x0F) as u8;
    size >>= 4;

    while size > 0 {
        output.push(byte | 0x80); // MSB = 1 means more bytes follow
        byte = (size & 0x7F) as u8;
        size >>= 7;
    }

    output.push(byte); // Final byte with MSB = 0
}

fn serialize_tree_entries(entries: &[crate::git::objects::TreeEntry]) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();

    for entry in entries {
        // Format: "<mode> <name>\0<20-byte-hash>"
        data.extend(entry.mode.as_bytes());
        data.push(b' ');
        data.extend(entry.name.as_bytes());
        data.push(0); // null terminator

        // Convert hex hash to binary
        let hash_bytes =
            hex::decode(&entry.hash).map_err(|_| format!("Invalid hash: {}", entry.hash))?;
        if hash_bytes.len() != 20 {
            return Err(format!("Invalid hash length: {}", entry.hash));
        }
        data.extend(&hash_bytes);
    }

    Ok(data)
}

fn serialize_commit_data(
    tree: &str,
    parents: &[String],
    author: &str,
    committer: &str,
    message: &str,
) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();

    // Format: "tree <hash>\nauthor <author>\ncommitter <committer>\n\n<message>"
    data.extend(format!("tree {}\n", tree).as_bytes());

    for parent_hash in parents {
        data.extend(format!("parent {}\n", parent_hash).as_bytes());
    }

    data.extend(format!("author {}\n", author).as_bytes());
    data.extend(format!("committer {}\n", committer).as_bytes());
    data.extend(b"\n"); // blank line before message
    data.extend(message.as_bytes());

    Ok(data)
}

fn serialize_tag_data(
    object: &str,
    tag_type: &str,
    tagger: &str,
    message: &str,
) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();

    data.extend(format!("object {}\n", object).as_bytes());
    data.extend(format!("type {}\n", tag_type).as_bytes());
    // Note: tag name would need to be stored separately in GitObject::Tag if needed
    data.extend(format!("tagger {}\n", tagger).as_bytes());
    data.extend(b"\n");
    data.extend(message.as_bytes());

    Ok(data)
}

// ============================================================================
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

// Sideband encoding functions
fn encode_sideband_data(band: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 1 + payload.len());
    let len_total = 4 /*header*/ + 1 /*band*/ + payload.len();
    out.extend(format!("{len_total:04x}").as_bytes()); // <-- include the 4 bytes!
    out.push(band);
    out.extend(payload);
    out
}

fn encode_progress_message(message: &[u8]) -> Vec<u8> {
    encode_sideband_data(2, message) // Band 2 = progress/status messages
}

fn encode_status_message(message: &[u8]) -> Vec<u8> {
    encode_sideband_data(1, message) // Band 1 = status messages
}
