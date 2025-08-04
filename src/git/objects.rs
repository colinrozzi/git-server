use crate::protocol::http::encode_pack_object_header;
use crate::utils::compression::compress_zlib;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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

    /// Get the Git object type as a byte
    pub fn object_type_byte(&self) -> u8 {
        match self {
            GitObject::Blob { .. } => 1,   // OBJ_BLOB
            GitObject::Tree { .. } => 2,   // OBJ_TREE
            GitObject::Commit { .. } => 3, // OBJ_COMMIT
            GitObject::Tag { .. } => 4,    // OBJ_TAG
        }
    }

    pub fn serialize_for_pack(&self) -> Result<Vec<u8>, String> {
        let obj_data = self.serialize_obj();

        let mut result = Vec::new();
        encode_pack_object_header(&mut result, self.object_type_byte(), obj_data.len());

        let compressed_data = compress_zlib(&obj_data);
        result.extend(compressed_data);
        Ok(result)
    }

    fn serialize_obj(&self) -> Vec<u8> {
        match self {
            GitObject::Blob { content } => {
                let mut data = vec![1]; // OBJ_BLOB = 1
                data.extend_from_slice(content);
                data
            }
            GitObject::Tree { entries } => {
                let mut data = Vec::new(); // OBJ_TREE = 2
                for entry in entries {
                    data.extend(entry.mode.as_bytes());
                    data.push(b' ');
                    data.extend(entry.name.as_bytes());
                    data.push(0);
                    data.extend_from_slice(
                        &hex::decode(&entry.hash)
                            .map_err(|_| format!("Invalid hash: {}", entry.hash))
                            .unwrap(),
                    );
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
                let mut data = vec![3]; // OBJ_COMMIT = 3
                data.extend_from_slice(tree.as_bytes());
                for parent in parents {
                    data.push(b' ');
                    data.extend_from_slice(parent.as_bytes());
                }
                data.push(b'\n');
                data.extend_from_slice(author.as_bytes());
                data.push(b'\n');
                data.extend_from_slice(committer.as_bytes());
                data.push(b'\n');
                data.extend_from_slice(message.as_bytes());
                data
            }
            GitObject::Tag {
                object,
                tag_type,
                tagger,
                message,
            } => {
                let mut data = vec![4]; // OBJ_TAG = 4
                data.extend_from_slice(object.as_bytes());
                data.push(b' ');
                data.extend_from_slice(tag_type.as_bytes());
                data.push(b'\n');
                data.extend_from_slice(tagger.as_bytes());
                data.push(b'\n');
                data.extend_from_slice(message.as_bytes());
                data
            }
        }
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
                    message
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

#[cfg(test)]
mod tests {}

