use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GitObject {
    Blob { content: Vec<u8> },
    Tree { entries: Vec<TreeEntry> },
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

    /// Get the Git pack file type number
    pub fn pack_type(&self) -> u8 {
        match self {
            GitObject::Blob { .. } => 1,    // OBJ_BLOB
            GitObject::Tree { .. } => 2,    // OBJ_TREE
            GitObject::Commit { .. } => 3,  // OBJ_COMMIT
            GitObject::Tag { .. } => 4,     // OBJ_TAG
        }
    }
}

impl TreeEntry {
    /// Create a new tree entry
    pub fn new(mode: String, name: String, hash: String) -> Self {
        Self { mode, name, hash }
    }

    /// Create a blob file entry (mode 100644)
    pub fn blob(name: String, hash: String) -> Self {
        Self::new("100644".to_string(), name, hash)
    }

    /// Create a directory entry (mode 040000)
    pub fn tree(name: String, hash: String) -> Self {
        Self::new("040000".to_string(), name, hash)
    }

    /// Create an executable file entry (mode 100755)
    pub fn executable(name: String, hash: String) -> Self {
        Self::new("100755".to_string(), name, hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_types() {
        let blob = GitObject::Blob { content: vec![1, 2, 3] };
        assert_eq!(blob.object_type(), "blob");
        assert_eq!(blob.pack_type(), 1);

        let tree = GitObject::Tree { entries: vec![] };
        assert_eq!(tree.object_type(), "tree");
        assert_eq!(tree.pack_type(), 2);

        let commit = GitObject::Commit {
            tree: "abc123".to_string(),
            parents: vec![],
            author: "Test <test@example.com>".to_string(),
            committer: "Test <test@example.com>".to_string(),
            message: "Test commit".to_string(),
        };
        assert_eq!(commit.object_type(), "commit");
        assert_eq!(commit.pack_type(), 3);
    }

    #[test]
    fn test_tree_entry_constructors() {
        let blob_entry = TreeEntry::blob("README.md".to_string(), "abc123".to_string());
        assert_eq!(blob_entry.mode, "100644");
        assert_eq!(blob_entry.name, "README.md");

        let dir_entry = TreeEntry::tree("src".to_string(), "def456".to_string());
        assert_eq!(dir_entry.mode, "040000");
        assert_eq!(dir_entry.name, "src");

        let exec_entry = TreeEntry::executable("script.sh".to_string(), "ghi789".to_string());
        assert_eq!(exec_entry.mode, "100755");
        assert_eq!(exec_entry.name, "script.sh");
    }
}
