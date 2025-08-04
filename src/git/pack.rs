//! Git Pack File Parsing and Generation
//!
//! This module handles packing and unpacking of Git objects
//! for efficient transfer over the wire protocol.
//!
//! REFACTORED: Now uses the new serialization architecture where
//! both loose and pack formats share the same content serialization
//! to ensure consistent SHA-1 hashes.

use super::objects::{GitObject, PackSerializer};
use crate::bindings::theater::simple::runtime::log;
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Pack file header
pub const PACK_SIGNATURE: &[u8; 4] = b"PACK";
pub const PACK_VERSION: u32 = 2;

/// Object types in pack files
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
    OfDelta,
    RefDelta,
}

impl ObjectType {
    pub fn from_pack_type(pack_type: u8) -> Option<Self> {
        match pack_type {
            1 => Some(ObjectType::Commit),
            2 => Some(ObjectType::Tree),
            3 => Some(ObjectType::Blob),
            4 => Some(ObjectType::Tag),
            6 => Some(ObjectType::OfDelta),
            7 => Some(ObjectType::RefDelta),
            _ => None,
        }
    }

    pub fn to_pack_type(&self) -> u8 {
        match self {
            ObjectType::Commit => 1,
            ObjectType::Tree => 2,
            ObjectType::Blob => 3,
            ObjectType::Tag => 4,
            ObjectType::OfDelta => 6,
            ObjectType::RefDelta => 7,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            ObjectType::Commit => "commit",
            ObjectType::Tree => "tree",
            ObjectType::Blob => "blob",
            ObjectType::Tag => "tag",
            ObjectType::OfDelta => "ofs-delta",
            ObjectType::RefDelta => "ref-delta",
        }
    }
}

/// Parsed Git object from pack file (intermediate representation)
#[derive(Debug, Clone)]
pub struct PackObject {
    pub obj_type: ObjectType,
    pub data: Vec<u8>,
    #[allow(dead_code)]
    pub base_offset: Option<u64>, // For delta objects
    #[allow(dead_code)]
    pub ref_hash: Option<String>, // For ref-delta objects
}

/// Git pack file parser
pub struct PackParser<'a> {
    data: &'a [u8],
    offset: usize,
    #[allow(dead_code)]
    objects: Vec<PackObject>,
}

impl<'a> PackParser<'a> {
    /// Create a new pack parser from byte data
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            offset: 0,
            objects: Vec::new(),
        }
    }

    /// Parse the entire pack file
    pub fn parse(&mut self) -> Result<Vec<GitObject>, String> {
        self.parse_header()?;
        let object_count = self.parse_object_count()?;

        log(&format!("Parsing pack file with {} objects", object_count));

        let mut objects = Vec::new();

        for _ in 0..object_count {
            let pack_obj = self.parse_object()?;

            // Convert PackObject to GitObject using new architecture
            let git_obj = self.convert_pack_object(pack_obj)?;
            objects.push(git_obj);
        }

        // Skip checksum for now
        let _ = self.verify_checksum();

        log(&format!("Parsed {} objects successfully", objects.len()));

        Ok(objects)
    }

    /// Parse the pack file header
    fn parse_header(&mut self) -> Result<(), String> {
        if self.data.len() < 8 {
            return Err("Pack file too small for header".to_string());
        }

        let signature = &self.data[..4];
        if signature != PACK_SIGNATURE {
            return Err(format!(
                "Invalid pack signature: expected {:?}, got {:?}",
                PACK_SIGNATURE, signature
            ));
        }

        let version = u32::from_be_bytes([self.data[4], self.data[5], self.data[6], self.data[7]]);

        if version != PACK_VERSION {
            return Err(format!("Unsupported pack version: {}", version));
        }

        self.offset = 8;
        Ok(())
    }

    /// Parse the object count from the header
    fn parse_object_count(&mut self) -> Result<u32, String> {
        if self.data.len() < self.offset + 4 {
            return Err("Pack file too small for object count".to_string());
        }

        let count = u32::from_be_bytes([
            self.data[self.offset],
            self.data[self.offset + 1],
            self.data[self.offset + 2],
            self.data[self.offset + 3],
        ]);

        self.offset += 4; // advance past the object-count field
        Ok(count)
    }

    /// Parse a single object from the pack file
    fn parse_object(&mut self) -> Result<PackObject, String> {
        if self.offset >= self.data.len() {
            return Err("Unexpected end of pack file".to_string());
        }

        let (obj_type, _size) = self.parse_object_header()?;

        // Read compressed data
        let compressed_data = &self.data[self.offset..];
        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut decompressed_data = Vec::new();
        decoder
            .read_to_end(&mut decompressed_data)
            .map_err(|e| format!("Failed to decompress object: {}", e))?;

        // Update offset
        let compressed_len = decoder.total_in() as usize;
        self.offset += compressed_len;

        Ok(PackObject {
            obj_type,
            data: decompressed_data,
            base_offset: None,
            ref_hash: None,
        })
    }

    /// Parse the object header (type and size)
    fn parse_object_header(&mut self) -> Result<(ObjectType, usize), String> {
        if self.offset >= self.data.len() {
            return Err("Unexpected end of data".to_string());
        }

        let mut byte = self.data[self.offset];
        let obj_type_num = (byte >> 4) & 0x07;
        let mut size = (byte & 0x0f) as usize;

        let mut shift = 4;
        self.offset += 1;

        // Handle variable-length size encoding
        while byte & 0x80 != 0 {
            if self.offset >= self.data.len() {
                return Err("Unexpected end of data".to_string());
            }
            byte = self.data[self.offset];
            size |= ((byte & 0x7f) as usize) << shift;
            shift += 7;
            self.offset += 1;
        }

        let obj_type = ObjectType::from_pack_type(obj_type_num)
            .ok_or_else(|| format!("Invalid object type: {}", obj_type_num))?;

        Ok((obj_type, size))
    }

    /// Convert PackObject to GitObject using new architecture
    fn convert_pack_object(&self, pack_obj: PackObject) -> Result<GitObject, String> {
        match pack_obj.obj_type {
            ObjectType::OfDelta | ObjectType::RefDelta => {
                Err("Delta objects not supported in basic implementation".to_string())
            }
            _ => {
                // Use the new PackSerializer for consistent deserialization
                let pack_type = pack_obj.obj_type.to_pack_type();
                
                // We need to re-compress the data because PackSerializer expects compressed data
                // In a real implementation, we'd avoid this double compression by refactoring
                let compressed_data = crate::utils::compression::compress_zlib(&pack_obj.data);
                
                PackSerializer::deserialize_object(pack_type, &compressed_data)
            }
        }
    }

    /// Verify pack file checksum
    fn verify_checksum(&mut self) -> Result<(), String> {
        // Skip checksum validation for now - focus on core functionality
        Ok(())
    }
}

/// High-level function to parse a pack file and return Git objects
pub fn parse_pack_file(data: &[u8]) -> Result<Vec<GitObject>, String> {
    let mut parser = PackParser::new(data);
    parser.parse()
}

/// Generate a pack file from a collection of Git objects
pub fn generate_pack_file(objects: &[GitObject]) -> Result<Vec<u8>, String> {
    let mut pack_data = Vec::new();
    
    // Pack header
    pack_data.extend_from_slice(PACK_SIGNATURE);
    pack_data.extend_from_slice(&PACK_VERSION.to_be_bytes());
    pack_data.extend_from_slice(&(objects.len() as u32).to_be_bytes());
    
    // Pack objects using new serialization architecture
    for obj in objects {
        let obj_data = obj.to_pack_format()?;
        pack_data.extend(obj_data);
    }
    
    // TODO: Add SHA-1 checksum of pack data
    // For now, add dummy checksum (20 zero bytes)
    pack_data.extend_from_slice(&[0u8; 20]);
    
    Ok(pack_data)
}

/// Pack file generator for streaming large object sets
pub struct PackGenerator {
    objects: Vec<GitObject>,
}

impl PackGenerator {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    pub fn add_object(&mut self, obj: GitObject) {
        self.objects.push(obj);
    }

    pub fn generate(&self) -> Result<Vec<u8>, String> {
        generate_pack_file(&self.objects)
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
}

impl Default for PackGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_type_from_pack() {
        assert_eq!(ObjectType::from_pack_type(1), Some(ObjectType::Commit));
        assert_eq!(ObjectType::from_pack_type(2), Some(ObjectType::Tree));
        assert_eq!(ObjectType::from_pack_type(3), Some(ObjectType::Blob));
        assert_eq!(ObjectType::from_pack_type(9), None);
    }

    #[test]
    fn test_pack_generation() {
        let obj = GitObject::Blob {
            content: b"hello world".to_vec(),
        };

        let mut generator = PackGenerator::new();
        generator.add_object(obj);

        let pack_data = generator.generate().unwrap();

        // Should have pack header (12 bytes) + object data + checksum (20 bytes)
        assert!(pack_data.len() > 32);
        assert_eq!(&pack_data[0..4], PACK_SIGNATURE);
    }

    #[test]
    fn test_round_trip_serialization() {
        let original = GitObject::Commit {
            tree: "abc123def4567890123456789012345678901234".to_string(),
            parents: vec![],
            author: "Test <test@example.com> 1234567890 +0000".to_string(),
            committer: "Test <test@example.com> 1234567890 +0000".to_string(),
            message: "Test message\n".to_string(),
        };

        // Test that hash is consistent between formats
        let hash1 = original.compute_hash();
        
        // Test loose format round trip
        let loose_data = original.to_loose_format();
        let from_loose = GitObject::from_loose_format(&loose_data).unwrap();
        let hash2 = from_loose.compute_hash();

        // Hashes should be consistent
        assert_eq!(hash1, hash2);
        assert_eq!(original, from_loose);
    }
}
