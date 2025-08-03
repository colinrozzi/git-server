//! Protocol v2 Receive-pack Binary Parser
//!
//! Correct implementation for Git Protocol v2 receive-pack requests
//! Addresses the "protocol v2 not implemented yet" error

use crate::git::repository::GitRepoState;
use crate::utils::logging::safe_log as log;

/// Structure to hold ref updates from push
#[derive(Debug, Clone)]
pub struct RefUpdate {
    pub ref_name: String,
    pub old_oid: String,
    pub new_oid: String,
}

/// Complete Protocol v2 push request
#[derive(Debug)]
pub struct PushRequest {
    pub ref_updates: Vec<RefUpdate>,
    pub pack_data: Vec<u8>,
    pub capabilities: Vec<String>,
}

/// Protocol v2 parser for receive-pack binary format
pub struct ProtocolV2Parser;

impl ProtocolV2Parser {
    /// Parse complete Protocol v2 receive-pack request
    pub fn parse_receive_pack_request(data: &[u8]) -> Result<PushRequest, String> {
        log("Parsing Protocol v2 receive-pack request");

        if data.is_empty() {
            return Err("Empty request body".to_string());
        }

        let mut cursor = 0;
        let mut capabilities = Vec::new();
        let mut ref_updates = Vec::new();

        // Phase 1: Parse packet-lines until PACK header
        let mut pack_start_pos = 0;

        // First, skip any extraneous data and find actual protocol data
        while cursor < data.len() - 4 {
            let remaining = &data[cursor..];

            // Check if we hit the PACK signature
            if remaining.starts_with(b"PACK") {
                pack_start_pos = cursor;
                break;
            }

            // Check if we have enough data for packet length
            if cursor + 4 > data.len() {
                break;
            }

            let len_str = std::str::from_utf8(&data[cursor..cursor + 4]).unwrap_or("0000");

            if let Ok(packet_len) = u16::from_str_radix(len_str.trim(), 16) {
                let packet_len = packet_len as usize;

                if packet_len == 0 {
                    // Flush packet
                    cursor += 4;
                    continue;
                }

                if packet_len >= 4 && cursor + packet_len <= data.len() {
                    let content = &data[cursor + 4..cursor + packet_len];
                    let text = std::str::from_utf8(content)
                        .unwrap_or("")
                        .trim_end_matches('\n');

                    // Parse ref updates that look like: old-oid new-oid ref-name
                    println!("Parsing packet content: '{}'  (len: {})", text, text.len());
                    if text.contains(' ') {
                        let parts: Vec<&str> = text.split_whitespace().collect();
                        println!("Split into {} parts: {:?}", parts.len(), parts);
                        if parts.len() >= 3 {
                            println!("Part lengths: {} {} {}", parts[0].len(), parts[1].len(), parts.get(2).map_or(0, |s| s.len()));
                        }
                        if parts.len() == 3 && parts[0].len() == 40 && parts[1].len() == 40 {
                            println!("Adding ref update: {} {} -> {}", parts[2], parts[0], parts[1]);
                            ref_updates.push(RefUpdate {
                                ref_name: parts[2].to_string(),
                                old_oid: parts[0].to_string(),
                                new_oid: parts[1].to_string(),
                            });
                        } else {
                            println!("Ref update conditions not met");
                        }
                    }

                    cursor += packet_len;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // If we found PACK signature, that's our pack data
        let pack_data = if pack_start_pos > 0 {
            data[pack_start_pos..].to_vec()
        } else {
            // Find PACK anywhere in the data
            if let Some(pack_pos) = data.windows(4).position(|w| w == b"PACK") {
                data[pack_pos..].to_vec()
            } else {
                // No pack data found - this might be a no-op push
                Vec::new()
            }
        };

        log(&format!(
            "Found {} ref updates and {} bytes of pack data",
            ref_updates.len(),
            pack_data.len()
        ));

        Ok(PushRequest {
            ref_updates,
            pack_data,
            capabilities,
        })
    }

    /// Helper to validate push requirements
    pub fn validate_push_request(
        ref_updates: &[RefUpdate],
        repo_state: &GitRepoState,
    ) -> Result<(), String> {
        // Check if we're creating new refs in empty repository
        if repo_state.refs.is_empty() && !ref_updates.is_empty() {
            log("Empty repository - accepting first push");
            return Ok(());
        }

        for update in ref_updates {
            if update.old_oid.len() != 40 || update.new_oid.len() != 40 {
                return Err("Invalid OID format".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ref_update() {
        let test_data = b"00660000000000000000000000000000000000000000 000340e325d1b85b3c0d5d7d8c5d46efad08fcd8 refs/heads/main\n0000PACK...";
        let result = ProtocolV2Parser::parse_receive_pack_request(test_data);
        assert!(result.is_ok());

        if let Ok(request) = result {
            assert_eq!(request.ref_updates.len(), 1);
            assert_eq!(request.ref_updates[0].ref_name, "refs/heads/main");
            assert!(request.ref_updates[0].old_oid.chars().all(|c| c == '0'));
        }
    }

    #[test]
    fn test_empty_repository_push() {
        let test_data = b"0066000340e325d1b85b3c0d5d7d8c5d46efad08fcd8 0000000000000000000000000000000000000000 refs/heads/main\n0000PACK...";
        
        println!("Test data: {:?}", std::str::from_utf8(test_data).unwrap_or("invalid utf8"));
        println!("Test data length: {}", test_data.len());
        
        let result = ProtocolV2Parser::parse_receive_pack_request(test_data);
        
        match &result {
            Ok(request) => {
                println!("Success! Ref updates: {}", request.ref_updates.len());
                for (i, update) in request.ref_updates.iter().enumerate() {
                    println!("  Update {}: {} {} -> {}", i, update.ref_name, update.old_oid, update.new_oid);
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
        
        assert!(result.is_ok());

        if let Ok(request) = result {
            assert_eq!(request.ref_updates.len(), 1);
            assert_eq!(request.ref_updates[0].ref_name, "refs/heads/main");
        }
    }
}

