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

