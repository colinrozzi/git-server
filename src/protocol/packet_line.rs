/// Git packet-line protocol implementation
/// 
/// The packet-line format is used throughout Git's wire protocols:
/// - Each line is prefixed with its length in 4-byte hex
/// - Special packets: "0000" (flush), "0001" (delimiter), "0002" (response-end)
/// - Maximum packet size: 65520 bytes (65516 data + 4 length)

/// Format a string as a Git packet-line
/// 
/// Packet format: 4-byte hex length + content
/// Length includes the 4-byte length field itself
pub fn format_pkt_line(line: &str) -> Vec<u8> {
    let len = line.len() + 4;
    let len_hex = format!("{:04x}", len);
    let mut result = len_hex.into_bytes();
    result.extend(line.as_bytes());
    result
}

/// Create a flush packet (signals end of data stream)
pub fn flush_packet() -> Vec<u8> {
    b"0000".to_vec()
}

/// Create a delimiter packet (used in protocol v2)
pub fn delimiter_packet() -> Vec<u8> {
    b"0001".to_vec()
}

/// Create a response-end packet (used in protocol v2)
pub fn response_end_packet() -> Vec<u8> {
    b"0002".to_vec()
}

/// Parse packet-line format from a byte stream
/// Returns (packet_content, bytes_consumed) or None if incomplete/invalid
pub fn parse_packet_line(data: &[u8], offset: usize) -> Option<(Vec<u8>, usize)> {
    if offset + 4 > data.len() {
        return None; // Not enough data for length header
    }
    
    let len_str = String::from_utf8_lossy(&data[offset..offset + 4]);
    let packet_len = match u16::from_str_radix(&len_str, 16) {
        Ok(len) => len as usize,
        Err(_) => return None,
    };
    
    if packet_len == 0 {
        // Flush packet
        return Some((vec![], 4));
    }
    
    if packet_len < 4 {
        return None; // Invalid packet length
    }
    
    if offset + packet_len > data.len() {
        return None; // Not enough data for full packet
    }
    
    // Extract packet content (excluding 4-byte length prefix)
    let content = data[offset + 4..offset + packet_len].to_vec();
    Some((content, packet_len))
}

/// Parse all packet-lines from a byte stream
/// Returns vector of packet contents
pub fn parse_all_packets(data: &[u8]) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let mut pos = 0;
    
    while pos < data.len() {
        match parse_packet_line(data, pos) {
            Some((content, consumed)) => {
                if !content.is_empty() {
                    packets.push(content);
                }
                pos += consumed;
            }
            None => break,
        }
    }
    
    packets
}

/// Format pack data in packet-line chunks
/// Used for streaming large pack files over Git Smart HTTP
pub fn format_pack_data(pack_data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    const MAX_PACKET_SIZE: usize = 65516; // 65520 - 4 byte header
    let mut pos = 0;
    
    while pos < pack_data.len() {
        let chunk_size = std::cmp::min(MAX_PACKET_SIZE, pack_data.len() - pos);
        let total_size = chunk_size + 4; // +4 for length header
        
        // Format as packet-line: 4-byte hex length + data
        let len_hex = format!("{:04x}", total_size);
        result.extend(len_hex.as_bytes());
        result.extend(&pack_data[pos..pos + chunk_size]);
        
        pos += chunk_size;
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_pkt_line() {
        let line = "hello world";
        let packet = format_pkt_line(line);
        
        // Should be: "000f" (15 in hex) + "hello world"
        assert_eq!(packet, b"000fhello world");
        assert_eq!(packet.len(), 15);
    }

    #[test]
    fn test_flush_packet() {
        let flush = flush_packet();
        assert_eq!(flush, b"0000");
        assert_eq!(flush.len(), 4);
    }

    #[test]
    fn test_parse_packet_line() {
        let data = b"000fhello world0000";
        
        // Parse first packet
        let (content, consumed) = parse_packet_line(data, 0).unwrap();
        assert_eq!(content, b"hello world");
        assert_eq!(consumed, 15);
        
        // Parse flush packet
        let (content, consumed) = parse_packet_line(data, 15).unwrap();
        assert_eq!(content, b"");
        assert_eq!(consumed, 4);
    }

    #[test]
    fn test_parse_all_packets() {
        // Create proper packet-line data:
        // "000f" (15 bytes) + "hello world" (11 bytes) = 15 total
        // "0008" (8 bytes) + "test" (4 bytes) = 8 total  
        // "0000" (flush)
        let data = b"000fhello world0008test0000";
        let packets = parse_all_packets(data);
        
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0], b"hello world");
        assert_eq!(packets[1], b"test");
    }

    #[test]
    fn test_invalid_packet() {
        let data = b"xxxx"; // Invalid hex length
        assert!(parse_packet_line(data, 0).is_none());
        
        let data = b"0003x"; // Length too short
        assert!(parse_packet_line(data, 0).is_none());
        
        let data = b"000f"; // Incomplete packet
        assert!(parse_packet_line(data, 0).is_none());
    }

    #[test]
    fn test_format_pack_data() {
        let pack_data = b"small pack";
        let formatted = format_pack_data(pack_data);
        
        // Should be packet-line formatted
        assert!(formatted.starts_with(b"000e")); // 14 bytes total (4 + 10)
        assert!(formatted.ends_with(b"small pack"));
    }
}
