use crate::git::repository::GitRepoState;
use crate::git::objects::GitObject;
use crate::utils::logging::safe_log as log;
use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};

/// Create an HTTP response with proper headers
pub fn create_response(status: u16, content_type: &str, body: &[u8]) -> HttpResponse {
    let headers = vec![
        ("Content-Type".to_string(), content_type.to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
    ];
    
    HttpResponse {
        status,
        headers,
        body: Some(body.to_vec()),
    }
}

/// Handle GET /info/refs (dumb HTTP)
/// Returns a simple text file listing all refs
pub fn handle_dumb_info_refs(repo_state: &GitRepoState) -> HttpResponse {
    log("Serving /info/refs (dumb HTTP)");
    
    let mut content = String::new();
    
    // List all refs in the format: <hash>\t<ref-name>\n
    for (ref_name, hash) in &repo_state.refs {
        content.push_str(&format!("{}\t{}\n", hash, ref_name));
    }
    
    // If no refs, return empty (valid for empty repository)
    if content.is_empty() {
        log("No refs to serve - empty repository");
    }
    
    create_response(200, "text/plain; charset=utf-8", content.as_bytes())
}

/// Handle GET /HEAD
/// Returns the contents of the HEAD file
pub fn handle_dumb_head(repo_state: &GitRepoState) -> HttpResponse {
    log("Serving /HEAD (dumb HTTP)");
    
    // HEAD file format: "ref: refs/heads/main\n"
    let head_content = format!("ref: {}\n", repo_state.head);
    
    create_response(200, "text/plain; charset=utf-8", head_content.as_bytes())
}

/// Handle GET /objects/xx/xxxxxx...
/// Serves individual Git objects
pub fn handle_dumb_object(repo_state: &GitRepoState, uri: &str) -> HttpResponse {
    log(&format!("Serving object: {}", uri));
    
    // Extract object hash from path like "/objects/ab/cdef123..."
    let hash = match extract_object_hash_from_path(uri) {
        Some(h) => h,
        None => {
            log(&format!("Invalid object path: {}", uri));
            return create_response(400, "text/plain", b"Invalid object path");
        }
    };
    
    log(&format!("Looking for object: {}", hash));
    
    if let Some(object) = repo_state.objects.get(&hash) {
        // Serialize the object in Git's loose object format
        let object_data = serialize_loose_object(object);
        create_response(200, "application/x-git-loose-object", &object_data)
    } else {
        log(&format!("Object not found: {}", hash));
        create_response(404, "text/plain", b"Object not found")
    }
}

/// Handle GET /refs/heads/branch or /refs/tags/tag
/// Serves individual ref files
pub fn handle_dumb_ref(repo_state: &GitRepoState, uri: &str) -> HttpResponse {
    log(&format!("Serving ref: {}", uri));
    
    // Convert URI path to ref name (remove leading slash)
    let ref_name = &uri[1..]; // Remove leading "/"
    
    if let Some(hash) = repo_state.refs.get(ref_name) {
        // Ref file contains just the hash + newline
        let ref_content = format!("{}\n", hash);
        create_response(200, "text/plain; charset=utf-8", ref_content.as_bytes())
    } else {
        log(&format!("Ref not found: {}", ref_name));
        create_response(404, "text/plain", b"Ref not found")
    }
}

/// Handle PUT /objects/xx/xxxxxx...
/// Allows uploading objects for push operations
pub fn handle_dumb_object_upload(repo_state: &mut GitRepoState, uri: &str, request: &HttpRequest) -> HttpResponse {
    log(&format!("Uploading object: {}", uri));
    
    let hash = match extract_object_hash_from_path(uri) {
        Some(h) => h,
        None => {
            return create_response(400, "text/plain", b"Invalid object path");
        }
    };
    
    let body = match &request.body {
        Some(data) => data,
        None => {
            return create_response(400, "text/plain", b"Missing object data");
        }
    };
    
    // Parse the uploaded Git loose object
    match parse_loose_object(body) {
        Ok(git_object) => {
            // Verify the hash matches
            let calculated_hash = crate::utils::hash::calculate_git_hash(&git_object);
            if calculated_hash != hash {
                log(&format!("Hash mismatch: expected {}, got {}", hash, calculated_hash));
                return create_response(400, "text/plain", b"Hash mismatch");
            }
            
            // Store the object
            repo_state.add_object(hash.clone(), git_object);
            log(&format!("Successfully stored object {}", hash));
            
            create_response(200, "text/plain", b"Object uploaded")
        }
        Err(e) => {
            log(&format!("Failed to parse object {}: {}", hash, e));
            create_response(400, "text/plain", format!("Failed to parse object: {}", e).as_bytes())
        }
    }
}

/// Handle PUT /refs/heads/branch
/// Updates a ref to point to a new commit
pub fn handle_dumb_ref_update(repo_state: &mut GitRepoState, uri: &str, request: &HttpRequest) -> HttpResponse {
    log(&format!("Updating ref: {}", uri));
    
    let ref_name = &uri[1..]; // Remove leading "/"
    
    let body = match &request.body {
        Some(data) => data,
        None => {
            return create_response(400, "text/plain", b"Missing ref data");
        }
    };
    
    let new_hash = String::from_utf8_lossy(body).trim().to_string();
    
    // Validate hash format (40 hex characters)
    if new_hash.len() != 40 || !new_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        log(&format!("Invalid hash format: {}", new_hash));
        return create_response(400, "text/plain", b"Invalid hash format");
    }
    
    // Verify the object exists
    if !repo_state.objects.contains_key(&new_hash) {
        log(&format!("Object {} not found", new_hash));
        return create_response(400, "text/plain", b"Object not found");
    }
    
    // Check if this is a force update or fast-forward
    let old_hash = repo_state.refs.get(ref_name);
    if let Some(old_hash) = old_hash {
        log(&format!("Updating ref {} from {} to {}", ref_name, old_hash, new_hash));
        // TODO: Add fast-forward check for safety
    } else {
        log(&format!("Creating new ref {} pointing to {}", ref_name, new_hash));
    }
    
    // Update the reference
    repo_state.refs.insert(ref_name.to_string(), new_hash.clone());
    
    // Update HEAD if this is the main branch
    if ref_name == "refs/heads/main" && repo_state.head == "refs/heads/main" {
        log("Updated main branch, HEAD remains the same");
    }
    
    create_response(200, "text/plain", b"Ref updated")
}

/// Extract object hash from path like "/objects/ab/cdef123..."
fn extract_object_hash_from_path(path: &str) -> Option<String> {
    // Path format: /objects/ab/cdef123456789...
    if !path.starts_with("/objects/") {
        return None;
    }
    
    let remaining = &path[9..]; // Remove "/objects/"
    let parts: Vec<&str> = remaining.split('/').collect();
    
    if parts.len() == 2 && parts[0].len() == 2 && parts[1].len() == 38 {
        // Reconstruct full hash: first 2 chars + remaining 38 chars
        Some(format!("{}{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Serialize a GitObject as a loose object (zlib compressed with header)
fn serialize_loose_object(object: &GitObject) -> Vec<u8> {
    use crate::utils::compression::compress_zlib;
    
    let (obj_type, content) = match object {
        GitObject::Blob { content } => ("blob", content.clone()),
        GitObject::Tree { entries } => {
            use crate::git::repository::serialize_tree_object;
            ("tree", serialize_tree_object(entries))
        },
        GitObject::Commit { tree, parents, author, committer, message } => {
            use crate::git::repository::serialize_commit_object;
            ("commit", serialize_commit_object(tree, parents, author, committer, message))
        },
        GitObject::Tag { .. } => {
            // TODO: Implement tag serialization
            ("tag", vec![])
        }
    };
    
    // Git loose object format: "<type> <size>\0<content>"
    let header = format!("{} {}\0", obj_type, content.len());
    let mut full_data = header.into_bytes();
    full_data.extend(content);
    
    // Compress with zlib
    compress_zlib(&full_data)
}

/// Parse a Git loose object from compressed data
fn parse_loose_object(compressed_data: &[u8]) -> Result<GitObject, String> {
    // Decompress the object
    let decompressed = crate::utils::compression::decompress_zlib(compressed_data)
        .map_err(|e| format!("Decompression failed: {}", e))?;
    
    // Find the null byte that separates header from content
    let null_pos = decompressed.iter().position(|&b| b == 0)
        .ok_or("No null separator found in object")?;
    
    // Parse header: "<type> <size>"
    let header = String::from_utf8_lossy(&decompressed[..null_pos]);
    let content = &decompressed[null_pos + 1..];
    
    let header_parts: Vec<&str> = header.split(' ').collect();
    if header_parts.len() != 2 {
        return Err(format!("Invalid object header: {}", header));
    }
    
    let obj_type = header_parts[0];
    let size: usize = header_parts[1].parse()
        .map_err(|_| format!("Invalid size in header: {}", header_parts[1]))?;
    
    if content.len() != size {
        return Err(format!("Size mismatch: header says {}, got {}", size, content.len()));
    }
    
    // Parse the object based on type
    match obj_type {
        "blob" => Ok(GitObject::Blob { content: content.to_vec() }),
        "tree" => parse_tree_object(content),
        "commit" => parse_commit_object(content),
        "tag" => parse_tag_object(content),
        _ => Err(format!("Unknown object type: {}", obj_type)),
    }
}

/// Parse a Git tree object
fn parse_tree_object(content: &[u8]) -> Result<GitObject, String> {
    let mut entries = Vec::new();
    let mut pos = 0;
    
    while pos < content.len() {
        // Find the space after mode
        let space_pos = content[pos..].iter().position(|&b| b == b' ')
            .ok_or("No space found after mode")?;
        let mode = String::from_utf8_lossy(&content[pos..pos + space_pos]).to_string();
        pos += space_pos + 1;
        
        // Find the null byte after filename
        let null_pos = content[pos..].iter().position(|&b| b == 0)
            .ok_or("No null byte found after filename")?;
        let name = String::from_utf8_lossy(&content[pos..pos + null_pos]).to_string();
        pos += null_pos + 1;
        
        // Read the 20-byte SHA-1 hash
        if pos + 20 > content.len() {
            return Err("Truncated hash in tree entry".to_string());
        }
        let hash = hex::encode(&content[pos..pos + 20]);
        pos += 20;
        
        entries.push(crate::git::objects::TreeEntry::new(mode, name, hash));
    }
    
    Ok(GitObject::Tree { entries })
}

/// Parse a Git commit object
fn parse_commit_object(content: &[u8]) -> Result<GitObject, String> {
    let content_str = String::from_utf8_lossy(content);
    let lines: Vec<&str> = content_str.lines().collect();
    
    let mut tree = String::new();
    let mut parents = Vec::new();
    let mut author = String::new();
    let mut committer = String::new();
    let mut message_start = 0;
    
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("tree ") {
            tree = line[5..].to_string();
        } else if line.starts_with("parent ") {
            parents.push(line[7..].to_string());
        } else if line.starts_with("author ") {
            author = line[7..].to_string();
        } else if line.starts_with("committer ") {
            committer = line[10..].to_string();
        } else if line.is_empty() {
            message_start = i + 1;
            break;
        }
    }
    
    let message = lines[message_start..].join("\n");
    
    if tree.is_empty() {
        return Err("No tree found in commit".to_string());
    }
    
    Ok(GitObject::Commit {
        tree,
        parents,
        author,
        committer,
        message,
    })
}

/// Parse a Git tag object
fn parse_tag_object(content: &[u8]) -> Result<GitObject, String> {
    let content_str = String::from_utf8_lossy(content);
    let lines: Vec<&str> = content_str.lines().collect();
    
    let mut object = String::new();
    let mut tag_type = String::new();
    let mut tagger = String::new();
    let mut message_start = 0;
    
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("object ") {
            object = line[7..].to_string();
        } else if line.starts_with("type ") {
            tag_type = line[5..].to_string();
        } else if line.starts_with("tagger ") {
            tagger = line[7..].to_string();
        } else if line.is_empty() {
            message_start = i + 1;
            break;
        }
    }
    
    let message = lines[message_start..].join("\n");
    
    Ok(GitObject::Tag {
        object,
        tag_type,
        tagger,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_object_hash() {
        assert_eq!(
            extract_object_hash_from_path("/objects/ab/cdef1234567890123456789012345678901234"),
            Some("abcdef1234567890123456789012345678901234".to_string())
        );
        
        assert_eq!(extract_object_hash_from_path("/invalid/path"), None);
        assert_eq!(extract_object_hash_from_path("/objects/ab/short"), None);
    }
    
    #[test]
    fn test_parse_blob_object() {
        // Create a simple blob object: "blob 13\0Hello, world!"
        let mut data = b"blob 13\0".to_vec();
        data.extend(b"Hello, world!");
        
        let compressed = crate::utils::compression::compress_zlib(&data);
        let result = parse_loose_object(&compressed).unwrap();
        
        match result {
            GitObject::Blob { content } => {
                assert_eq!(content, b"Hello, world!");
            }
            _ => panic!("Expected blob object"),
        }
    }
    
    #[test]
    fn test_parse_commit_object() {
        let commit_content = b"tree abc123def456\nauthor Test User <test@example.com> 1234567890 +0000\ncommitter Test User <test@example.com> 1234567890 +0000\n\nInitial commit\n";
        
        let mut data = format!("commit {}\0", commit_content.len()).into_bytes();
        data.extend(commit_content);
        
        let compressed = crate::utils::compression::compress_zlib(&data);
        let result = parse_loose_object(&compressed).unwrap();
        
        match result {
            GitObject::Commit { tree, message, .. } => {
                assert_eq!(tree, "abc123def456");
                assert_eq!(message, "Initial commit");
            }
            _ => panic!("Expected commit object"),
        }
    }
}
