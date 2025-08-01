use super::packet_line::{format_pkt_line, flush_packet};
use super::negotiation::{parse_upload_pack_request, generate_negotiation_response, determine_objects_to_send, UploadPackRequest};
use crate::git::repository::GitRepoState;
use crate::git::pack::{generate_pack_file, generate_empty_pack};
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

/// Extract query parameter from URI
pub fn extract_query_param(uri: &str, param: &str) -> Option<String> {
    if let Some(query_start) = uri.find('?') {
        let query = &uri[query_start + 1..];
        for pair in query.split('&') {
            if let Some(eq_pos) = pair.find('=') {
                let key = &pair[..eq_pos];
                let value = &pair[eq_pos + 1..];
                if key == param {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Handle Git Smart HTTP info/refs discovery request
/// 
/// This is the first phase of Git Smart HTTP protocol where the client
/// discovers what refs (branches/tags) are available on the server.
pub fn handle_info_refs(repo_state: &GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Handling /info/refs request");
    
    // Parse query parameters to get the service
    let service = extract_query_param(&request.uri, "service");
    
    match service.as_deref() {
        Some("git-upload-pack") => {
            log("Info/refs for git-upload-pack (clone/fetch)");
            handle_upload_pack_discovery(repo_state)
        }
        Some("git-receive-pack") => {
            log("Info/refs for git-receive-pack (push)");
            handle_receive_pack_discovery(repo_state)
        }
        _ => {
            log(&format!("Unknown service parameter: {:?}", service));
            create_response(400, "text/plain", "Bad Request: missing or invalid service parameter".as_bytes())
        }
    }
}

/// Generate upload-pack discovery response (for clone/fetch)
/// 
/// Response format:
/// - Service announcement
/// - List of refs with their commit hashes
/// - Capabilities on first ref line
pub fn handle_upload_pack_discovery(repo_state: &GitRepoState) -> HttpResponse {
    log("Generating upload-pack advertisement");
    
    let mut response_body = Vec::new();
    
    // Service announcement
    let service_line = "# service=git-upload-pack\n";
    response_body.extend(format_pkt_line(service_line));
    response_body.extend(flush_packet()); // Flush packet
    
    // Advertise refs
    for (ref_name, commit_hash) in &repo_state.refs {
        let ref_line = format!("{} {}\n", commit_hash, ref_name);
        response_body.extend(format_pkt_line(&ref_line));
    }
    
    response_body.extend(flush_packet()); // End of refs
    
    log(&format!("Upload-pack discovery response: {} bytes", response_body.len()));
    
    create_response(
        200,
        "application/x-git-upload-pack-advertisement",
        &response_body
    )
}

/// Generate receive-pack discovery response (for push)
/// 
/// Similar to upload-pack but includes push-specific capabilities
pub fn handle_receive_pack_discovery(repo_state: &GitRepoState) -> HttpResponse {
    log("Generating receive-pack advertisement");
    
    let mut response_body = Vec::new();
    
    // Service announcement
    let service_line = "# service=git-receive-pack\n";
    response_body.extend(format_pkt_line(service_line));
    response_body.extend(flush_packet()); // Flush packet
    
    // Advertise refs with capabilities
    let mut first_ref = true;
    for (ref_name, commit_hash) in &repo_state.refs {
        let ref_line = if first_ref {
            first_ref = false;
            format!("{} {}\0report-status delete-refs side-band-64k\n", commit_hash, ref_name)
        } else {
            format!("{} {}\n", commit_hash, ref_name)
        };
        response_body.extend(format_pkt_line(&ref_line));
    }
    
    response_body.extend(flush_packet()); // End of refs
    
    log(&format!("Receive-pack discovery response: {} bytes", response_body.len()));
    
    create_response(
        200,
        "application/x-git-receive-pack-advertisement",
        &response_body
    )
}

/// Handle upload-pack data transfer (clone/fetch)
/// 
/// This is the main data transfer phase where:
/// 1. Client sends want/have negotiation
/// 2. Server responds with ACK/NAK
/// 3. Server sends pack file with requested objects
pub fn handle_upload_pack(repo_state: &mut GitRepoState, request: &HttpRequest) -> HttpResponse {
    log("Handling upload-pack request (clone/fetch data transfer)");
    
    // Parse the request body to extract want/have lines
    let request_body = request.body.as_ref().map(|b| b.as_slice()).unwrap_or(&[]);
    let negotiation = parse_upload_pack_request(request_body);
    
    log(&format!("Client wants {} objects, has {} objects", 
                 negotiation.want_count(), negotiation.have_count()));
    
    // Determine what objects to send
    let objects_to_send = if negotiation.wants_everything() {
        // Client wants full clone, send all refs
        log("Client wants full clone");
        repo_state.refs.values()
            .filter(|hash| *hash != "0000000000000000000000000000000000000000")
            .cloned()
            .collect()
    } else {
        // Determine specific objects based on wants
        determine_objects_to_send(
            &negotiation,
            |_hash| None, // We don't use this callback in current implementation
            |hash| repo_state.refs.values().any(|h| h == hash)
        )
    };
    
    // Build the response
    let mut response_body = Vec::new();
    
    // Phase 1: Negotiation response
    let negotiation_response = generate_negotiation_response(
        &negotiation,
        |hash| repo_state.refs.values().any(|h| h == hash) || repo_state.objects.contains_key(hash)
    );
    response_body.extend(negotiation_response);
    
    // Phase 2: Pack file transfer
    log("Generating pack file");
    
    // If no specific objects requested, send all objects (for fresh clone)
    let final_objects = if objects_to_send.is_empty() {
        log("No specific objects requested, sending all objects");
        repo_state.refs.values()
            .filter(|hash| *hash != "0000000000000000000000000000000000000000")
            .cloned()
            .collect()
    } else {
        objects_to_send
    };
    
    // Generate and send pack file
    if !final_objects.is_empty() {
        let pack_data = generate_pack_file(repo_state, &final_objects);
        // For git clone, pack data is sent directly after negotiation (not in packet-line format)
        response_body.extend(pack_data);
    } else {
        log("No objects to send, sending empty pack");
        let empty_pack = generate_empty_pack();
        response_body.extend(empty_pack);
    }
    
    log(&format!("Upload-pack response: {} bytes", response_body.len()));
    
    create_response(
        200,
        "application/x-git-upload-pack-result",
        &response_body
    )
}

/// Handle receive-pack data transfer (push)
/// 
/// TODO: Implement push protocol
/// - Parse incoming pack file
/// - Update refs
/// - Send status report
pub fn handle_receive_pack(_repo_state: &mut GitRepoState, _request: &HttpRequest) -> HttpResponse {
    log("Handling receive-pack request (push data transfer)");
    
    // For now, return a minimal response
    // TODO: Parse pack file from request body
    // TODO: Update refs based on push
    
    let response_body = flush_packet(); // Empty response for now
    
    create_response(
        200,
        "application/x-git-receive-pack-result",
        &response_body
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::objects::{GitObject, TreeEntry};

    #[test]
    fn test_extract_query_param() {
        let uri = "/info/refs?service=git-upload-pack&other=value";
        
        assert_eq!(extract_query_param(uri, "service"), Some("git-upload-pack".to_string()));
        assert_eq!(extract_query_param(uri, "other"), Some("value".to_string()));
        assert_eq!(extract_query_param(uri, "missing"), None);
        
        // Test URI without query string
        let uri_no_query = "/info/refs";
        assert_eq!(extract_query_param(uri_no_query, "service"), None);
    }

    #[test]
    fn test_create_response() {
        let body = b"test content";
        let response = create_response(200, "text/plain", body);
        
        assert_eq!(response.status, 200);
        assert_eq!(response.body, Some(body.to_vec()));
        
        // Check headers
        let headers: std::collections::HashMap<String, String> = response.headers.into_iter().collect();
        assert_eq!(headers.get("Content-Type"), Some(&"text/plain".to_string()));
        assert_eq!(headers.get("Content-Length"), Some(&"12".to_string()));
    }

    #[test]
    fn test_upload_pack_discovery() {
        let mut repo = GitRepoState::new("test".to_string());
        repo.update_ref("refs/heads/main".to_string(), "abc123def456".to_string());
        
        let response = handle_upload_pack_discovery(&repo);
        
        assert_eq!(response.status, 200);
        
        let body = response.body.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        
        // Should contain service announcement
        assert!(body_str.contains("# service=git-upload-pack"));
        
        // Should contain ref advertisement
        assert!(body_str.contains("abc123def456 refs/heads/main"));
    }

    #[test]
    fn test_receive_pack_discovery() {
        let mut repo = GitRepoState::new("test".to_string());
        repo.update_ref("refs/heads/main".to_string(), "abc123def456".to_string());
        
        let response = handle_receive_pack_discovery(&repo);
        
        assert_eq!(response.status, 200);
        
        let body = response.body.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        
        // Should contain service announcement
        assert!(body_str.contains("# service=git-receive-pack"));
        
        // Should contain capabilities on first ref
        assert!(body_str.contains("report-status"));
    }
}
