use super::packet_line::parse_all_packets;
use crate::utils::logging::safe_log as log;

/// Represents a Git upload-pack request with want/have negotiation
#[derive(Debug, Clone)]
pub struct UploadPackRequest {
    pub wants: Vec<String>,
    pub haves: Vec<String>,
    pub capabilities: Vec<String>,
}

impl UploadPackRequest {
    /// Create a new empty upload-pack request
    pub fn new() -> Self {
        Self {
            wants: Vec::new(),
            haves: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// Check if this is a fresh clone (no haves)
    pub fn is_fresh_clone(&self) -> bool {
        self.haves.is_empty()
    }

    /// Check if client wants everything (zero hash)
    pub fn wants_everything(&self) -> bool {
        self.wants.iter().any(|want| want == "0000000000000000000000000000000000000000")
    }

    /// Get the number of objects wanted
    pub fn want_count(&self) -> usize {
        self.wants.len()
    }

    /// Get the number of objects the client has
    pub fn have_count(&self) -> usize {
        self.haves.len()
    }
}

impl Default for UploadPackRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse an upload-pack request from the raw request body
/// 
/// The request body contains packet-line formatted want/have lines:
/// - want <sha1> [<capability-list>]
/// - have <sha1>
/// - done (optional)
pub fn parse_upload_pack_request(body: &[u8]) -> UploadPackRequest {
    let mut request = UploadPackRequest::new();
    
    log(&format!("Parsing upload-pack request: {} bytes", body.len()));
    
    // Parse all packet-lines from the body
    let packets = parse_all_packets(body);
    
    for packet in packets {
        let line = String::from_utf8_lossy(&packet);
        let line = line.trim();
        
        if line.starts_with("want ") {
            parse_want_line(line, &mut request);
        } else if line.starts_with("have ") {
            parse_have_line(line, &mut request);
        } else if line == "done" {
            // Client finished sending haves
            log("Client sent 'done' - negotiation complete");
        }
    }
    
    log(&format!("Parsed: {} wants, {} haves, {} capabilities", 
                 request.wants.len(), request.haves.len(), request.capabilities.len()));
    
    request
}

/// Parse a "want" line: want <sha1> [<capability-list>]
fn parse_want_line(line: &str, request: &mut UploadPackRequest) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let hash = parts[1].to_string();
        request.wants.push(hash);
        
        // Parse capabilities from first want line only
        if parts.len() > 2 && request.wants.len() == 1 {
            for cap in &parts[2..] {
                request.capabilities.push(cap.to_string());
            }
        }
    }
}

/// Parse a "have" line: have <sha1>
fn parse_have_line(line: &str, request: &mut UploadPackRequest) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        request.haves.push(parts[1].to_string());
    }
}

/// Generate negotiation response for upload-pack
/// 
/// Returns packet-line formatted response with ACK/NAK
pub fn generate_negotiation_response(
    request: &UploadPackRequest,
    have_common: impl Fn(&str) -> bool,
) -> Vec<u8> {
    use super::packet_line::{format_pkt_line, flush_packet};
    
    let mut response = Vec::new();
    
    if request.is_fresh_clone() {
        // First clone - no negotiation needed, client has nothing
        // Send NAK to end negotiation phase  
        response.extend(format_pkt_line("NAK\n"));
        log("Sent NAK for fresh clone");
    } else {
        // Handle have/want negotiation
        let mut found_common = false;
        
        for have_hash in &request.haves {
            if have_common(have_hash) {
                let ack_line = format!("ACK {}\n", have_hash);
                response.extend(format_pkt_line(&ack_line));
                found_common = true;
                log(&format!("ACK for common object: {}", have_hash));
            }
        }
        
        if !found_common {
            response.extend(format_pkt_line("NAK\n"));
            log("NAK - no common objects found");
        }
    }
    
    response
}

/// Determine what objects to send based on wants/haves
/// 
/// Returns list of object hashes that should be included in the pack
pub fn determine_objects_to_send(
    request: &UploadPackRequest,
    get_ref_hash: impl Fn(&str) -> Option<String>,
    has_ref: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut objects_to_send = Vec::new();
    
    for want_hash in &request.wants {
        // Zero hash means client wants everything (fresh clone)
        if want_hash == "0000000000000000000000000000000000000000" {
            log("Client wants full clone (zero hash)");
            // This should be handled by the caller to send all refs
            continue;
        } else if has_ref(want_hash) {
            // We have this specific ref, add it to objects to send
            objects_to_send.push(want_hash.clone());
            log(&format!("Will send object: {}", want_hash));
        } else {
            log(&format!("Missing object: {}", want_hash));
        }
    }
    
    objects_to_send
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_request() {
        let request = UploadPackRequest::new();
        assert!(request.is_fresh_clone());
        assert!(!request.wants_everything());
        assert_eq!(request.want_count(), 0);
        assert_eq!(request.have_count(), 0);
    }

    #[test]
    fn test_parse_want_line() {
        let mut request = UploadPackRequest::new();
        parse_want_line("want abc123def456 capability1 capability2", &mut request);
        
        assert_eq!(request.wants.len(), 1);
        assert_eq!(request.wants[0], "abc123def456");
        assert_eq!(request.capabilities.len(), 2);
        assert_eq!(request.capabilities[0], "capability1");
        assert_eq!(request.capabilities[1], "capability2");
    }

    #[test]
    fn test_parse_have_line() {
        let mut request = UploadPackRequest::new();
        parse_have_line("have def456abc789", &mut request);
        
        assert_eq!(request.haves.len(), 1);
        assert_eq!(request.haves[0], "def456abc789");
    }

    #[test]
    fn test_wants_everything() {
        let mut request = UploadPackRequest::new();
        request.wants.push("0000000000000000000000000000000000000000".to_string());
        
        assert!(request.wants_everything());
    }

    #[test]
    fn test_fresh_clone_vs_incremental() {
        let fresh = UploadPackRequest::new();
        assert!(fresh.is_fresh_clone());
        
        let mut incremental = UploadPackRequest::new();
        incremental.haves.push("abc123".to_string());
        assert!(!incremental.is_fresh_clone());
    }

    #[test]
    fn test_determine_objects_to_send() {
        let mut request = UploadPackRequest::new();
        request.wants.push("abc123".to_string());
        request.wants.push("def456".to_string());
        request.wants.push("0000000000000000000000000000000000000000".to_string());
        
        let objects = determine_objects_to_send(
            &request,
            |_| Some("hash".to_string()),
            |hash| hash == "abc123" || hash == "def456"
        );
        
        // Should include abc123 and def456, but skip the zero hash
        assert_eq!(objects.len(), 2);
        assert!(objects.contains(&"abc123".to_string()));
        assert!(objects.contains(&"def456".to_string()));
    }

    #[test]
    fn test_negotiation_response() {
        let mut request = UploadPackRequest::new();
        request.haves.push("common123".to_string());
        request.haves.push("missing456".to_string());
        
        let response = generate_negotiation_response(
            &request,
            |hash| hash == "common123"
        );
        
        // Should contain ACK for common123 and NAK overall
        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("ACK common123"));
    }
}
