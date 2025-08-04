use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use flate2::Compression;
use std::io::{Write, Read};

/// High-performance zlib compression for Git pack files using flate2
/// 
/// Git requires object data in pack files to be compressed with zlib (RFC 1950).
/// This implementation uses the flate2 library with the high-performance zlib-rs backend
/// for maximum speed while maintaining full Git compatibility.
pub fn compress_zlib(data: &[u8]) -> Vec<u8> {
    // Use flate2's ZlibEncoder with default compression level
    // The zlib-rs backend provides excellent performance
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    
    // Write all data to the encoder
    encoder.write_all(data).expect("Writing to ZlibEncoder should never fail");
    
    // Finish compression and return the compressed bytes
    encoder.finish().expect("Finishing ZlibEncoder should never fail")
}

/// High-performance zlib decompression for Git objects
/// 
/// Decompresses zlib-compressed data (RFC 1950) as used in Git loose objects.
/// Uses the same high-performance flate2 library with zlib-rs backend.
#[allow(dead_code)]
pub fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();
    decoder.read_to_end(&mut result)?;
    Ok(result)
}

/// Calculate Adler-32 checksum (now mainly for compatibility/testing)
/// 
/// Note: flate2 handles checksums internally, but we keep this function
/// for any external code that might need Adler-32 calculation
#[allow(dead_code)]
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
    fn test_compress_zlib_basic() {
        let data = b"Hello, World!";
        let compressed = compress_zlib(data);
        
        // Should produce valid zlib data (starts with zlib header)
        assert!(compressed.len() > data.len() + 6); // header + data + checksum
        
        // Test that it's valid zlib by decompressing
        use flate2::read::ZlibDecoder;
        use std::io::Read;
        
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        
        assert_eq!(decompressed, data);
    }
    
    #[test]
    fn test_compress_zlib_empty() {
        let compressed = compress_zlib(&[]);
        
        // Even empty data should compress to something
        assert!(compressed.len() > 0);
        
        // Verify it decompresses correctly
        use flate2::read::ZlibDecoder;
        use std::io::Read;
        
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        
        assert_eq!(decompressed, Vec::<u8>::new());
    }
    
    #[test]
    fn test_compress_zlib_large_data() {
        // Test with larger data to ensure good compression
        let data = b"This is a test string that should compress well because it has repetitive content. ".repeat(100);
        let compressed = compress_zlib(&data);
        
        // Should actually compress (be smaller than original)
        assert!(compressed.len() < data.len());
        
        // Verify correctness
        use flate2::read::ZlibDecoder;
        use std::io::Read;
        
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_adler32_known_values() {
        // Test against known Adler-32 values (kept for compatibility)
        assert_eq!(calculate_adler32(b""), 1);
        assert_eq!(calculate_adler32(b"a"), 0x00620062);
        assert_eq!(calculate_adler32(b"abc"), 0x024d0127);
        assert_eq!(calculate_adler32(b"Wikipedia"), 0x11E60398);
    }
    
    #[test]
    fn test_compression_performance() {
        // Test that flate2 actually compresses better than our old "stored" format
        let repetitive_data = b"ABCD".repeat(1000); // 4000 bytes of repetitive data
        let compressed = compress_zlib(&repetitive_data);
        
        // With actual compression, this should be much smaller
        assert!(compressed.len() < repetitive_data.len() / 10, 
               "Compressed size {} should be much less than original size {}", 
               compressed.len(), repetitive_data.len());
    }
    
    #[test]
    fn test_decompress_zlib() {
        let original = b"Hello, zlib decompression!";
        let compressed = compress_zlib(original);
        let decompressed = decompress_zlib(&compressed).unwrap();
        
        assert_eq!(decompressed, original);
    }
    
    #[test]
    fn test_compress_decompress_roundtrip() {
        let test_data = b"This is test data for compression roundtrip testing. It should compress and decompress perfectly.";
        
        let compressed = compress_zlib(test_data);
        let decompressed = decompress_zlib(&compressed).unwrap();
        
        assert_eq!(decompressed, test_data);
        assert!(compressed.len() < test_data.len()); // Should actually compress
    }
}
