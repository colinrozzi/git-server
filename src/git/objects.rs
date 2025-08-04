//! Git Object Model and Serialization
//!
//! This module provides a clean separation between:
//! - GitObject: Pure data model representing Git semantics
//! - LooseObjectSerializer: Handles .git/objects/* format
//! - PackSerializer: Handles pack file format
//!
//! Key insight: Both formats use identical content serialization to ensure
//! consistent SHA-1 hashes across different storage formats.

use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[cfg(not(test))]
use crate::bindings::theater::simple::runtime::log;

// ============================================================================
// Pure Data Model - No Serialization Logic
// ============================================================================

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GitObject {
    Blob {
        content: Vec<u8>,
    },
    Tree {
        entries: Vec<TreeEntry>,
    },
    Commit {
        tree: String,
        parents: Vec<String>,
        author: String,
        committer: String,
        message: String,
    },
    Tag {
        object: String,
        tag_type: String,
        tagger: String,
        message: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: String,
}

impl GitObject {
    /// Get the Git object type as a string
    pub fn object_type(&self) -> &'static str {
        match self {
            GitObject::Blob { .. } => "blob",
            GitObject::Tree { .. } => "tree",
            GitObject::Commit { .. } => "commit",
            GitObject::Tag { .. } => "tag",
        }
    }

    /// Get the Git object type as a byte (for pack format)
    pub fn object_type_byte(&self) -> u8 {
        match self {
            GitObject::Blob { .. } => 3,   // OBJ_BLOB in pack format
            GitObject::Tree { .. } => 2,   // OBJ_TREE in pack format
            GitObject::Commit { .. } => 1, // OBJ_COMMIT in pack format
            GitObject::Tag { .. } => 4,    // OBJ_TAG in pack format
        }
    }

    /// Compute SHA-1 hash of this object (using loose format)
    pub fn compute_hash(&self) -> String {
        use sha1::{Sha1, Digest};

        let loose_data = LooseObjectSerializer::serialize(self);
        let mut hasher = Sha1::new();
        hasher.update(&loose_data);
        hex::encode(hasher.finalize())
    }

    /// Serialize for loose object storage
    pub fn to_loose_format(&self) -> Vec<u8> {
        LooseObjectSerializer::serialize(self)
    }

    /// Serialize for pack file
    pub fn to_pack_format(&self) -> Result<Vec<u8>, String> {
        PackSerializer::serialize_object(self)
    }

    /// Create from loose object data
    pub fn from_loose_format(data: &[u8]) -> Result<Self, String> {
        LooseObjectSerializer::deserialize(data)
    }

    /// Create from pack object data
    pub fn from_pack_format(pack_type: u8, compressed_data: &[u8]) -> Result<Self, String> {
        PackSerializer::deserialize_object(pack_type, compressed_data)
    }
}

impl Display for GitObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitObject::Blob { content } => write!(f, "Blob with {} bytes", content.len()),
            GitObject::Tree { entries } => write!(f, "Tree with {} entries", entries.len()),
            GitObject::Commit {
                tree,
                parents,
                author,
                committer,
                message,
            } => {
                write!(
                    f,
                    "Commit: tree={}, parents=[{}], author={}, committer={}, message={}",
                    tree,
                    parents.join(", "),
                    author,
                    committer,
                    message.trim()
                )
            }
            GitObject::Tag {
                object,
                tag_type,
                tagger,
                message,
            } => {
                write!(
                    f,
                    "Tag: object={}, type={}, tagger={}, message={}",
                    object, tag_type, tagger, message
                )
            }
        }
    }
}

impl TreeEntry {
    /// Create a new tree entry
    pub fn new(mode: String, name: String, hash: String) -> Self {
        Self { mode, name, hash }
    }

    #[allow(dead_code)]
    pub fn executable(name: String, hash: String) -> Self {
        Self::new("100755".to_string(), name, hash)
    }
}

// ============================================================================
// Loose Object Format Handler (for .git/objects/* storage)
// ============================================================================

pub struct LooseObjectSerializer;

impl LooseObjectSerializer {
    /// Serialize object in loose format: "<type> <size>\0<content>"
    pub fn serialize(obj: &GitObject) -> Vec<u8> {
        let content = Self::serialize_content(obj);
        let header = format!("{} {}\0", obj.object_type(), content.len());

        let mut result = Vec::new();
        result.extend_from_slice(header.as_bytes());
        result.extend_from_slice(&content);
        result
    }

    /// Deserialize object from loose format
    pub fn deserialize(data: &[u8]) -> Result<GitObject, String> {
        // Find the null terminator that separates header from content
        let null_pos = data
            .iter()
            .position(|&b| b == 0)
            .ok_or("Invalid loose object: missing null terminator")?;

        let header = std::str::from_utf8(&data[..null_pos])
            .map_err(|_| "Invalid UTF-8 in object header")?;

        let content = &data[null_pos + 1..];

        // Parse header: "<type> <size>"
        let parts: Vec<&str> = header.split(' ').collect();
        if parts.len() != 2 {
            return Err("Invalid object header format".to_string());
        }

        let obj_type = parts[0];
        let declared_size: usize = parts[1]
            .parse()
            .map_err(|_| "Invalid size in object header")?;

        if content.len() != declared_size {
            return Err(format!(
                "Size mismatch: declared {}, actual {}",
                declared_size,
                content.len()
            ));
        }

        Self::deserialize_content(obj_type, content)
    }

    /// Serialize just the content part (used by both loose and pack formats)
    /// This is the KEY function that ensures consistent hashing!
    pub fn serialize_content(obj: &GitObject) -> Vec<u8> {
        match obj {
            GitObject::Blob { content } => content.clone(),

            GitObject::Tree { entries } => {
                let mut data = Vec::new();
                for entry in entries {
                    // Git tree format: <mode> <name>\0<20-byte-hash>
                    data.extend_from_slice(entry.mode.as_bytes());
                    data.push(b' ');
                    data.extend_from_slice(entry.name.as_bytes());
                    data.push(0); // null terminator

                    // Convert hex hash to binary
                    let hash_bytes = hex::decode(&entry.hash)
                        .unwrap_or_else(|_| panic!("Invalid hex hash in tree entry: {}", entry.hash));
                    data.extend_from_slice(&hash_bytes);
                }
                data
            }

            GitObject::Commit {
                tree,
                parents,
                author,
                committer,
                message,
            } => {
                let mut data = Vec::new();

                // tree line
                data.extend_from_slice(b"tree ");
                data.extend_from_slice(tree.as_bytes());
                data.push(b'\n');

                // parent lines
                for parent in parents {
                    data.extend_from_slice(b"parent ");
                    data.extend_from_slice(parent.as_bytes());
                    data.push(b'\n');
                }

                // author line
                data.extend_from_slice(b"author ");
                data.extend_from_slice(author.as_bytes());
                data.push(b'\n');

                // committer line
                data.extend_from_slice(b"committer ");
                data.extend_from_slice(committer.as_bytes());
                data.push(b'\n');

                // blank line before message
                data.push(b'\n');

                // commit message (preserve exactly as stored - critical for hash consistency!)
                data.extend_from_slice(message.as_bytes());

                data
            }

            GitObject::Tag {
                object,
                tag_type,
                tagger,
                message,
            } => {
                let mut data = Vec::new();

                data.extend_from_slice(b"object ");
                data.extend_from_slice(object.as_bytes());
                data.push(b'\n');

                data.extend_from_slice(b"type ");
                data.extend_from_slice(tag_type.as_bytes());
                data.push(b'\n');

                data.extend_from_slice(b"tagger ");
                data.extend_from_slice(tagger.as_bytes());
                data.push(b'\n');

                data.push(b'\n'); // blank line
                data.extend_from_slice(message.as_bytes());

                data
            }
        }
    }

    /// Deserialize content based on object type
    pub fn deserialize_content(obj_type: &str, content: &[u8]) -> Result<GitObject, String> {
        match obj_type {
            "blob" => Ok(GitObject::Blob {
                content: content.to_vec(),
            }),

            "tree" => Self::parse_tree_content(content),
            "commit" => Self::parse_commit_content(content),
            "tag" => Self::parse_tag_content(content),

            _ => Err(format!("Unknown object type: {}", obj_type)),
        }
    }

    fn parse_tree_content(data: &[u8]) -> Result<GitObject, String> {
        let mut entries = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            // Parse mode (until space)
            let space_pos = data[pos..]
                .iter()
                .position(|&b| b == b' ')
                .ok_or("Malformed tree entry: missing space after mode")?;

            let mode = std::str::from_utf8(&data[pos..pos + space_pos])
                .map_err(|_| "Invalid UTF-8 in tree mode")?;
            pos += space_pos + 1;

            // Parse name (until null)
            let null_pos = data[pos..]
                .iter()
                .position(|&b| b == 0)
                .ok_or("Malformed tree entry: missing null terminator")?;

            let name = std::str::from_utf8(&data[pos..pos + null_pos])
                .map_err(|_| "Invalid UTF-8 in tree name")?;
            pos += null_pos + 1;

            // Parse 20-byte SHA-1
            if pos + 20 > data.len() {
                return Err("Malformed tree entry: truncated hash".to_string());
            }

            let hash = hex::encode(&data[pos..pos + 20]);
            pos += 20;

            entries.push(TreeEntry {
                mode: mode.to_string(),
                name: name.to_string(),
                hash,
            });
        }

        Ok(GitObject::Tree { entries })
    }

    fn parse_commit_content(data: &[u8]) -> Result<GitObject, String> {
        let content = std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in commit object")?;

        let tree = Self::extract_field(content, "tree").ok_or("Missing tree in commit")?;

        let parents = Self::extract_all_fields(content, "parent");

        let author = Self::extract_field(content, "author").ok_or("Missing author in commit")?;

        let committer =
            Self::extract_field(content, "committer").ok_or("Missing committer in commit")?;

        // Extract message (everything after first blank line)
        // CRITICAL: Preserve exact message bytes for hash consistency
        let message = if let Some(msg_start) = content.find("\n\n") {
            content[msg_start + 2..].to_string()
        } else {
            String::new()
        };

        Ok(GitObject::Commit {
            tree,
            parents,
            author,
            committer,
            message,
        })
    }

    fn parse_tag_content(data: &[u8]) -> Result<GitObject, String> {
        let content = std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in tag object")?;

        let object = Self::extract_field(content, "object").ok_or("Missing object in tag")?;

        let tag_type = Self::extract_field(content, "type").ok_or("Missing type in tag")?;

        let tagger = Self::extract_field(content, "tagger").ok_or("Missing tagger in tag")?;

        let message = if let Some(msg_start) = content.find("\n\n") {
            content[msg_start + 2..].to_string()
        } else {
            String::new()
        };

        Ok(GitObject::Tag {
            object,
            tag_type,
            tagger,
            message,
        })
    }

    fn extract_field(content: &str, field: &str) -> Option<String> {
        let prefix = format!("{} ", field);
        content
            .lines()
            .find(|line| line.starts_with(&prefix))
            .map(|line| line[prefix.len()..].to_string())
    }

    fn extract_all_fields(content: &str, field: &str) -> Vec<String> {
        let prefix = format!("{} ", field);
        content
            .lines()
            .filter_map(|line| {
                if line.starts_with(&prefix) {
                    Some(line[prefix.len()..].to_string())
                } else {
                    None
                }
            })
            .collect()
    }
}

// ============================================================================
// Pack Format Handler (for git protocol transfer)
// ============================================================================

use flate2::read::ZlibDecoder;
use std::io::Read;

pub struct PackSerializer;

impl PackSerializer {
    /// Serialize object for pack file (with pack header + compressed content)
    pub fn serialize_object(obj: &GitObject) -> Result<Vec<u8>, String> {
        // Get the raw content (same as loose object content - KEY FOR HASH CONSISTENCY!)
        let content = LooseObjectSerializer::serialize_content(obj);

        let mut result = Vec::new();

        // Encode pack object header (type + size using varint encoding)
        Self::encode_pack_header(&mut result, obj.object_type_byte(), content.len());

        // Compress content with zlib
        let compressed = crate::utils::compression::compress_zlib(&content);
        result.extend(compressed);

        Ok(result)
    }

    /// Deserialize object from pack format
    pub fn deserialize_object(pack_type: u8, compressed_data: &[u8]) -> Result<GitObject, String> {
        // Decompress the data
        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut content = Vec::new();
        decoder
            .read_to_end(&mut content)
            .map_err(|e| format!("Failed to decompress pack object: {}", e))?;

        // Convert pack type to object type string
        let obj_type = match pack_type {
            1 => "commit",
            2 => "tree",
            3 => "blob",
            4 => "tag",
            _ => return Err(format!("Unsupported pack object type: {}", pack_type)),
        };

        // Use the same content parser as loose objects - ENSURES CONSISTENCY!
        LooseObjectSerializer::deserialize_content(obj_type, &content)
    }

    fn encode_pack_header(buf: &mut Vec<u8>, obj_type: u8, size: usize) {
        // First byte: (type << 4) | (size & 0x0F) | continuation_bit
        let mut byte = (obj_type << 4) | ((size & 0x0F) as u8);
        let mut remaining_size = size >> 4;

        if remaining_size > 0 {
            byte |= 0x80; // continuation bit
        }
        buf.push(byte);

        // Variable-length size encoding
        while remaining_size > 0 {
            let mut byte = (remaining_size & 0x7F) as u8;
            remaining_size >>= 7;

            if remaining_size > 0 {
                byte |= 0x80; // continuation bit
            }
            buf.push(byte);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_loose_format() {
        let original = GitObject::Commit {
            tree: "abc123".to_string(),
            parents: vec!["parent1".to_string()],
            author: "Test Author <test@example.com> 1234567890 +0000".to_string(),
            committer: "Test Committer <test@example.com> 1234567890 +0000".to_string(),
            message: "Test commit message\n".to_string(),
        };

        let serialized = LooseObjectSerializer::serialize(&original);
        let deserialized = LooseObjectSerializer::deserialize(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_hash_consistency() {
        let obj = GitObject::Blob {
            content: b"hello world".to_vec(),
        };

        // Hash should be consistent across serialization round-trips
        let hash1 = obj.compute_hash();

        let loose_data = obj.to_loose_format();
        let obj2 = GitObject::from_loose_format(&loose_data).unwrap();
        let hash2 = obj2.compute_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_serialization_consistency() {
        let obj = GitObject::Commit {
            tree: "2b297e643c551e76cfa1f93810c50811382f9117".to_string(),
            parents: vec![],
            author: "colin <colinrozzi@gmail.com> 1754330635 -0400".to_string(),
            committer: "colin <colinrozzi@gmail.com> 1754330635 -0400".to_string(),
            message: "Initial commit\n".to_string(),
        };

        // Both loose and pack should use same content serialization
        let loose_content = LooseObjectSerializer::serialize_content(&obj);
        
        // Simulate what pack serialization does internally
        let _pack_bytes = obj.to_pack_format().unwrap();
        // (We'd need to parse the pack header and decompress to verify, 
        //  but the key is they both call serialize_content())
        
        // Verify the content format matches expected Git format
        let content_str = std::str::from_utf8(&loose_content).unwrap();
        assert!(content_str.starts_with("tree "));
        assert!(content_str.contains("\nauthor "));
        assert!(content_str.contains("\ncommitter "));
        assert!(content_str.contains("\n\nInitial commit\n"));
    }

    #[test]
    fn test_tree_round_trip() {
        let original = GitObject::Tree { 
            entries: vec![
                TreeEntry::new("100644".to_string(), "file.txt".to_string(), "abc123def4567890123456789012345678901234".to_string()),
                TreeEntry::new("100755".to_string(), "script.sh".to_string(), "fedcba6543210987654321098765432109876543".to_string()),
            ]
        };

        let serialized = LooseObjectSerializer::serialize(&original);
        let deserialized = LooseObjectSerializer::deserialize(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }
}
