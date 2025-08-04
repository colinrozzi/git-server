// Git Protocol Handler - Supporting both v1 and v2
// FIXED: Handle Protocol v1 fallback for push operations

use crate::bindings::theater::simple::http_types::HttpResponse;
use crate::bindings::theater::simple::runtime::log;
use crate::git::repository::GitRepoState;

pub const CAPABILITIES: &str = "report-status delete-refs ofs-delta agent=git-server/0.1.0";
pub const MAX_PKT_PAYLOAD: usize = 0xFFF0 - 4; // pkt-line payload limit = 65 516
pub const MAX_SIDEBAND_DATA: usize = MAX_PKT_PAYLOAD - 1; // minus 1-byte channel

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

pub fn create_error_response(message: &str) -> HttpResponse {
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
    log(&format!("Collected objects: {:?}", objects_to_send));

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
            log(&format!("Processing object: {}", obj));
            let obj_data = serialize_object_for_pack(obj)?;
            pack.extend(&obj_data);
        } else {
            return Err(format!("Object not found: {}", obj_id));
        }
    }

    // Pack checksum (SHA1 of entire pack so far)
    let checksum = sha1_hash(&pack);
    pack.extend(&checksum);

    log(&format!("Generated packfile: {:?}", pack));
    Ok(pack)
}

pub fn serialize_object_for_pack(obj: &crate::git::objects::GitObject) -> Result<Vec<u8>, String> {
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

    log(&format!(
        "Object data: {}",
        String::from_utf8_lossy(&obj_data)
    ));

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
pub fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    let mut result = format!("{:04x}", total_len).into_bytes();
    result.extend_from_slice(data);
    result
}

pub fn encode_flush_pkt() -> Vec<u8> {
    b"0000".to_vec()
}

// Sideband encoding functions
pub fn encode_sideband_data(band: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 1 + payload.len());
    let len_total = 4 /*header*/ + 1 /*band*/ + payload.len();
    out.extend(format!("{len_total:04x}").as_bytes()); // <-- include the 4 bytes!
    out.push(band);
    out.extend(payload);
    out
}

pub fn encode_progress_message(message: &[u8]) -> Vec<u8> {
    encode_sideband_data(2, message) // Band 2 = progress/status messages
}

pub fn encode_status_message(message: &[u8]) -> Vec<u8> {
    encode_sideband_data(1, message) // Band 1 = status messages
}
