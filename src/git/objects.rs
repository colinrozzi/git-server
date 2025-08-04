use crate::protocol::http::encode_pack_object_header;
use crate::utils::compression::compress_zlib;
use std::fmt::Display;

#[cfg(not(test))]
use crate::bindings::theater::simple::runtime::log;

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
                // For blobs, the content is stored as-is (no prefixes)
                content.clone()
            }
            GitObject::Tree { entries } => {
                let mut data = Vec::new();
                for entry in entries {
                    // Git tree format: <mode> <name>\0<20-byte-hash>
                    data.extend(entry.mode.as_bytes());
                    data.push(b' ');
                    data.extend(entry.name.as_bytes());
                    data.push(0); // null terminator
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
                let mut data = Vec::new();

                // tree line
                data.extend_from_slice(b"tree ");
                data.extend_from_slice(tree.as_bytes());
                data.push(b'\n');

                // parent lines (if any)
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

                // commit message
                data.extend_from_slice(message.as_bytes());

                // Git commit objects must end with exactly one newline
                // If message doesn't end with newline, add one
                if !message.ends_with('\n') {
                    data.push(b'\n');
                }

                data
            }
            GitObject::Tag {
                object,
                tag_type,
                tagger,
                message,
            } => {
                let mut data = Vec::new();

                // object line
                data.extend_from_slice(b"object ");
                data.extend_from_slice(object.as_bytes());
                data.push(b'\n');

                // type line
                data.extend_from_slice(b"type ");
                data.extend_from_slice(tag_type.as_bytes());
                data.push(b'\n');

                // tagger line
                data.extend_from_slice(b"tagger ");
                data.extend_from_slice(tagger.as_bytes());
                data.push(b'\n');

                // blank line before message
                data.push(b'\n');

                // tag message
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
mod tests {
    use super::*;

    #[test]
    fn test_commit_serialization() {
        let commit = GitObject::Commit {
            tree: "2b297e643c551e76cfa1f93810c50811382f9117".to_string(),
            parents: vec![],
            author: "colin <colinrozzi@gmail.com> 1754330635 -0400".to_string(),
            committer: "colin <colinrozzi@gmail.com> 1754330635 -0400".to_string(),
            message: "Initial commit".to_string(),
        };

        let serialized = commit.serialize_obj();
        let as_string = String::from_utf8_lossy(&serialized);

        println!("Serialized commit:");
        println!("{}", as_string);

        // Check that it has the correct format
        assert!(as_string.starts_with("tree "));
        assert!(as_string.contains("\nauthor "));
        assert!(as_string.contains("\ncommitter "));
        assert!(as_string.contains("\n\nInitial commit"));

        let expected = "tree 2b297e643c551e76cfa1f93810c50811382f9117\nauthor colin <colinrozzi@gmail.com> 1754330635 -0400\ncommitter colin <colinrozzi@gmail.com> 1754330635 -0400\n\nInitial commit";
        assert_eq!(as_string, expected);
    }

    #[test]
    fn test_commit_with_parents() {
        let commit = GitObject::Commit {
            tree: "abc123".to_string(),
            parents: vec!["parent1".to_string(), "parent2".to_string()],
            author: "test <test@example.com> 1234567890 +0000".to_string(),
            committer: "test <test@example.com> 1234567890 +0000".to_string(),
            message: "Test commit".to_string(),
        };

        let serialized = commit.serialize_obj();
        let as_string = String::from_utf8_lossy(&serialized);

        println!("Commit with parents:");
        println!("{}", as_string);

        assert!(as_string.contains("parent parent1\n"));
        assert!(as_string.contains("parent parent2\n"));
    }
}
