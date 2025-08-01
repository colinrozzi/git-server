/// Simple SHA-1 implementation for Git pack checksums and object hashing
pub fn sha1_hash(data: &[u8]) -> [u8; 20] {
    let mut h = [
        0x67452301u32,
        0xEFCDAB89u32,
        0x98BADCFEu32,
        0x10325476u32,
        0xC3D2E1F0u32,
    ];
    
    let original_len = data.len();
    let mut message = data.to_vec();
    
    // Append the '1' bit (plus zero padding to make it a byte)
    message.push(0x80);
    
    // Append zeros until length ≡ 448 (mod 512)
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    
    // Append original length in bits as 64-bit big-endian integer
    let bit_len = (original_len as u64) * 8;
    message.extend(&bit_len.to_be_bytes());
    
    // Process message in 512-bit chunks
    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 80];
        
        // Break chunk into sixteen 32-bit big-endian words
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1], 
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        
        // Extend the words
        for i in 16..80 {
            w[i] = left_rotate(w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16], 1);
        }
        
        // Initialize hash value for this chunk
        let [mut a, mut b, mut c, mut d, mut e] = h;
        
        // Main loop
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                60..=79 => (b ^ c ^ d, 0xCA62C1D6),
                _ => unreachable!(),
            };
            
            let temp = left_rotate(a, 5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            
            e = d;
            d = c;
            c = left_rotate(b, 30);
            b = a;
            a = temp;
        }
        
        // Add this chunk's hash to result
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    
    // Convert to bytes
    let mut result = [0u8; 20];
    for (i, &word) in h.iter().enumerate() {
        let bytes = word.to_be_bytes();
        result[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    
    result
}

/// Calculate Git object hash in the standard format: SHA-1("<type> <size>\0<content>")
pub fn calculate_git_hash(obj_type: &str, content: &[u8]) -> String {
    let header = format!("{} {}\0", obj_type, content.len());
    
    let mut full_content = Vec::new();
    full_content.extend(header.as_bytes());
    full_content.extend(content);
    
    let hash_bytes = sha1_hash(&full_content);
    
    // Convert to hex string
    hash_bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// Left rotate operation for SHA-1
fn left_rotate(value: u32, bits: u32) -> u32 {
    (value << bits) | (value >> (32 - bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_known_values() {
        // Test against known Git object hashes
        let blob_content = b"hello world";
        let blob_hash = calculate_git_hash("blob", blob_content);
        // This should match: echo "hello world" | git hash-object --stdin
        assert_eq!(blob_hash, "95d09f2b10159347eece71399a7e2e907ea3df4f");
        
        // Test empty tree
        let empty_tree_hash = calculate_git_hash("tree", &[]);
        assert_eq!(empty_tree_hash, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    }
    
    #[test]
    fn test_sha1_basic() {
        // Test the basic SHA-1 implementation
        let result = sha1_hash(b"abc");
        let hex_result: String = result.iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        assert_eq!(hex_result, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
}
