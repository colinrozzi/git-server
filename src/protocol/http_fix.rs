// Clean, minimal Protocol v2 fixes for build issues
use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::git::repository::GitRepoState;
use crate::protocol::protocol_v2_parser::ProtocolV2Parser;
crate::utils::logging::safe_log as log;
use std::collections::HashSet;

/// Fixed receive-pack handler using new parser
pub fn handle_receive_pack_request_fixed(
    repo_state: &mut GitRepoState, 
    request: &HttpRequest
) -> HttpResponse {
    log("Handling Protocol v2 receive-pack");
    
    let body = match &request.body {
        Some(body) => body,
        None => return create_status_response(false, vec!["unpack missing-request".to_string()]),
    };
    
    match ProtocolV2Parser::parse_receive_pack_request(body) {
        Ok(push) => {
            log!("Parsing succeeded - {} ref updates, {} bytes pack", 
                 push.ref_updates.len(), push.pack_data.len());
            
            if push.ref_updates.is_empty() {
                return create_status_response(true, vec![]);
            }
            
            let ref_tuples: Vec<(String,String,String)> = push.ref_updates
                .into_iter()
                .map(|r| (r.ref_name, r.old_oid, r.new_oid))
                .collect();
                
            match repo_state.process_push_operation(&push.pack_data, ref_tuples) {
                Ok(statuses) => {
                    let ref_statuses: Vec<String> = statuses.into_iter()
                        .map(|s| s.strip_prefix("create ")
                               .or_else(|| s.strip_prefix("update "))
                               .map(|ref_name| format!("ok {}", ref_name))
                               .unwrap_or(s))
                        .collect();
                    create_status_response(true, ref_statuses)
                }
                Err(e) => create_status_response(false, vec![format!("unpack {}", e)])
            }
        }
        Err(e) => create_status_response(false, vec![format!("unpack {}", e)])
    }
}

/// Create status response with String types
pub fn create_status_response(success: bool, ref_statuses: Vec<String>) -> HttpResponse {
    let mut data = Vec::new();
    let unpack = if success { b"unpack ok\n" } else { b"unpack error\n" };
    data.extend_from_slice(b"0009");
    data.extend_from_slice(unpack);
    
    for status in ref_statuses {
        let line = format!("{}\n", status);
        data.extend(format!("{:04x}{}", line.len() + 4, line).as_bytes());
    }
    data.extend_from_slice(b"0000");
    
    super::http::create_response(200, "application/x-git-receive-pack-result", &data)
}