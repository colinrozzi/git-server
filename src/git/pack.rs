//! Git Pack File Parsing and Generation
//!
//! This module handles packing and unpacking of Git objects
//! for efficient transfer over the wire protocol.

use super::objects::{GitObject, TreeEntry};
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

/// Parsed Git object from pack file
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

            // Convert PackObject to GitObject
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

        self.offset += 4; // ⬅️  advance past the object-count field
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

    /// Convert PackObject to GitObject
    fn convert_pack_object(&self, pack_obj: PackObject) -> Result<GitObject, String> {
        let data = &pack_obj.data;

        match pack_obj.obj_type {
            ObjectType::Commit => self.parse_commit_objects(data),
            ObjectType::Tree => self.parse_tree_object(data),
            ObjectType::Blob => Ok(GitObject::Blob {
                content: data.to_vec(),
            }),
            ObjectType::Tag => {
                // Parse tag object (simple implementation)
                let content_str =
                    std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in tag object")?;

                let object = self
                    .extract_commit_field(content_str, "object")
                    .unwrap_or_else(|| "unknown".to_string());
                let tag_type = self
                    .extract_commit_field(content_str, "type")
                    .unwrap_or_else(|| "commit".to_string());
                let tagger = self
                    .extract_commit_field(content_str, "tagger")
                    .unwrap_or_else(|| "unknown".to_string());
                let message = self
                    .extract_commit_field(content_str, "")
                    .unwrap_or_else(|| content_str.to_string());

                Ok(GitObject::Tag {
                    object,
                    tag_type,
                    tagger,
                    message,
                })
            }
            ObjectType::OfDelta | ObjectType::RefDelta => {
                Err("Delta objects not supported in basic implementation".to_string())
            }
        }
    }

    /// Parse commit object
    fn parse_commit_objects(&self, data: &[u8]) -> Result<GitObject, String> {
        let content_str =
            std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in commit object")?;

        let tree = self
            .extract_commit_field(content_str, "tree")
            .ok_or("Missing tree in commit")?;

        let parents = self.extract_all_commit_fields(content_str, "parent");

        let author = self
            .extract_commit_field(content_str, "author")
            .unwrap_or_else(|| "unknown author".to_string());

        let committer = self
            .extract_commit_field(content_str, "committer")
            .unwrap_or_else(|| "unknown committer".to_string());

        let message = self
            .extract_commit_field(content_str, "")
            .unwrap_or_else(|| content_str.to_string());

        Ok(GitObject::Commit {
            tree,
            parents,
            author,
            committer,
            message,
        })
    }

    /// Parse tree object
    fn parse_tree_object(&self, data: &[u8]) -> Result<GitObject, String> {
        let mut entries = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            if pos + 1 >= data.len() {
                return Err("Truncated tree object".to_string());
            }

            // Parse mode (up to space)
            let space_pos = data[pos..].iter().position(|&b| b == b' ');
            let Some(space_pos) = space_pos else {
                return Err("Malformed tree entry".to_string());
            };

            let mode = std::str::from_utf8(&data[pos..pos + space_pos])
                .map_err(|_| "Invalid mode in tree")?;

            pos += space_pos + 1;

            if pos >= data.len() {
                return Err("Truncated tree entry".to_string());
            }

            // Parse name (up to null terminator)
            let null_pos = data[pos..].iter().position(|&b| b == b'\0');
            let Some(null_pos) = null_pos else {
                return Err("Malformed tree entry: missing null terminator".to_string());
            };

            let name = std::str::from_utf8(&data[pos..pos + null_pos])
                .map_err(|_| "Invalid name in tree")?;

            pos += null_pos + 1;

            if pos + 20 > data.len() {
                return Err("Truncated tree entry: missing sha1".to_string());
            }

            // Parse SHA-1 (20 bytes)
            let mut hash_bytes = [0u8; 20];
            hash_bytes.copy_from_slice(&data[pos..pos + 20]);
            pos += 20;

            // Convert binary to hex string
            let hash = hex::encode(hash_bytes);

            entries.push(TreeEntry::new(mode.to_string(), name.to_string(), hash));
        }

        Ok(GitObject::Tree { entries })
    }

    /// Helper to extract fields from commit message
    fn extract_commit_field(&self, content: &str, field: &str) -> Option<String> {
        if field.is_empty() {
            // Return message after headers
            if let Some(double_newline_idx) = content.find("\n\n") {
                let message = &content[double_newline_idx + 2..];
                return Some(message.trim().to_string());
            }
            return Some(content.to_string());
        }

        let prefix = format!("{} ", field);
        content
            .lines()
            .find(|line| line.starts_with(&prefix))
            .map(|line| line[prefix.len()..].trim().to_string())
    }

    /// Extract all occurrences of a commit field (like parent)
    fn extract_all_commit_fields(&self, content: &str, field: &str) -> Vec<String> {
        let prefix = format!("{} ", field);
        content
            .lines()
            .filter_map(|line| {
                if line.starts_with(&prefix) {
                    Some(line[prefix.len()..].trim().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Verify pack file checksum
    fn verify_checksum(&mut self) -> Result<(), String> {
        // Skip checksum validation for now - focus on core functionality
        // Skip checksum validation for now - focus on core functionality
        Ok(())
    }
}

/// High-level function to parse a pack file and return Git objects
pub fn parse_pack_file(data: &[u8]) -> Result<Vec<GitObject>, String> {
    let mut parser = PackParser::new(data);
    parser.parse()
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
    fn test_extract_commit_field() {
        let commit_content =
            b"tree abc123\nparent def456\nauthor someone\ncommitter nobody\n\nInitial commit\n";
        let parser = PackParser::new(&[]);
        let commit_str = std::str::from_utf8(commit_content).unwrap();

        assert_eq!(
            parser.extract_commit_field(commit_str, "tree"),
            Some("abc123".to_string())
        );
        assert_eq!(
            parser.extract_commit_field(commit_str, "parent"),
            Some("def456".to_string())
        );
        assert_eq!(
            parser.extract_all_commit_fields(commit_str, "parent"),
            vec!["def456"]
        );
    }
}

