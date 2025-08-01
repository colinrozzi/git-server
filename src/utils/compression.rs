/// Simple zlib compression implementation for Git pack files
/// 
/// Git requires object data in pack files to be compressed with zlib (RFC 1950).
/// This implementation creates valid zlib streams using uncompressed deflate blocks
/// for simplicity while maintaining compatibility.
pub fn compress_zlib(data: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::new();
    
    // zlib header (RFC 1950)
    // CMF (Compression Method and flags): 0x78 (deflate, 32K window)
    // FLG (flags): 0x9C (check bits to make header checksum correct)
    compressed.extend(&[0x78, 0x9C]);
    
    // For simplicity, use "stored" (uncompressed) deflate blocks
    // This is valid deflate format but not compressed
    
    let mut pos = 0;
    while pos < data.len() {
        let chunk_size = std::cmp::min(65535, data.len() - pos);
        let is_final = pos + chunk_size >= data.len();
        
        // Block header: BFINAL (1 bit) + BTYPE (2 bits) = 00000000 or 00000001
        // BTYPE 00 = stored (uncompressed)
        compressed.push(if is_final { 0x01 } else { 0x00 });
        
        // LEN (length of data) - little endian
        compressed.extend(&(chunk_size as u16).to_le_bytes());
        
        // NLEN (one's complement of LEN) - little endian  
        compressed.extend(&(!(chunk_size as u16)).to_le_bytes());
        
        // Raw data
        compressed.extend(&data[pos..pos + chunk_size]);
        
        pos += chunk_size;
    }
    
    // Adler-32 checksum of original data
    let adler32 = calculate_adler32(data);
    compressed.extend(&adler32.to_be_bytes());
    
    compressed
}

/// Calculate Adler-32 checksum as required by zlib format (RFC 1950)
pub fn calculate_adler32(data: &[u8]) -> u32 {
    const ADLER32_BASE: u32 = 65521;
    
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    
    for &byte in data {
        a = (a + byte as u32) % ADLER32_BASE;
        b = (b + a) % ADLER32_BASE;
    }
    
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adler32_known_values() {
        // Test against known Adler-32 values
        assert_eq!(calculate_adler32(b""), 1);
        assert_eq!(calculate_adler32(b"a"), 0x00620062);
        assert_eq!(calculate_adler32(b"abc"), 0x024d0127);
        assert_eq!(calculate_adler32(b"Wikipedia"), 0x11E60398);
    }
    
    #[test]
    fn test_zlib_header() {
        let compressed = compress_zlib(b"test");
        // Should start with zlib header
        assert_eq!(&compressed[0..2], &[0x78, 0x9C]);
        // Should end with 4-byte Adler-32 checksum
        assert!(compressed.len() >= 6); // header + data + checksum
    }
    
    #[test]
    fn test_empty_compression() {
        let compressed = compress_zlib(&[]);
        // Even empty data should have header + final block + checksum
        assert!(compressed.len() >= 6);
        assert_eq!(&compressed[0..2], &[0x78, 0x9C]);
    }
}
