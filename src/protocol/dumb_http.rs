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
    
    // For now, just acknowledge the upload
    // TODO: Parse the object data and store it properly
    log(&format!("Received object {} ({} bytes)", hash, body.len()));
    
    create_response(200, "text/plain", b"Object uploaded")
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
    if new_hash.len() == 40 && new_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        repo_state.refs.insert(ref_name.to_string(), new_hash.clone());
        log(&format!("Updated ref {} to {}", ref_name, new_hash));
        create_response(200, "text/plain", b"Ref updated")
    } else {
        log(&format!("Invalid hash format: {}", new_hash));
        create_response(400, "text/plain", b"Invalid hash format")
    }
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
}
