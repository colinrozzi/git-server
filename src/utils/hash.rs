use crate::git::objects::GitObject;
use sha1::{Digest, Sha1};

/// Calculate SHA-1 hash using the sha1 crate
pub fn sha1_hash(data: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Calculate SHA-1 hash and return as hex string
pub fn sha1_hex(data: &[u8]) -> String {
    let hash_bytes = sha1_hash(data);
    hex::encode(hash_bytes)
}

/// Calculate Git hash for a GitObject (convenience function)
pub fn calculate_git_hash(object: &GitObject) -> String {
    let (obj_type, content) = match object {
        GitObject::Blob { content } => ("blob", content.clone()),
        GitObject::Tree { entries } => (
            "tree",
            crate::git::repository::serialize_tree_object(entries),
        ),
        GitObject::Commit {
            tree,
            parents,
            author,
            committer,
            message,
        } => (
            "commit",
            crate::git::repository::serialize_commit_object(
                tree, parents, author, committer, message,
            ),
        ),
        GitObject::Tag {
            object,
            tag_type,
            tagger,
            message,
        } => {
            // Create tag object content
            let mut tag_content = format!("object {}\ntype {}\n", object, tag_type);
            if !tagger.is_empty() {
                tag_content.push_str(&format!("tagger {}\n", tagger));
            }
            tag_content.push('\n');
            tag_content.push_str(message);
            ("tag", tag_content.into_bytes())
        }
    };

    calculate_git_hash_raw(obj_type, &content)
}

/// Calculate Git hash for raw object type and content
pub fn calculate_git_hash_raw(obj_type: &str, content: &[u8]) -> String {
    let header = format!("{} {}\0", obj_type, content.len());

    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(content);

    let hash_bytes = hasher.finalize();

    // Convert to hex string using hex crate
    hex::encode(hash_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_known_values() {
        // Test against known Git object hashes
        let blob_content = b"hello world";
        let blob_hash = calculate_git_hash_raw("blob", blob_content);
        // This should match: echo "hello world" | git hash-object --stdin
        assert_eq!(blob_hash, "95d09f2b10159347eece71399a7e2e907ea3df4f");

        // Test empty tree
        let empty_tree_hash = calculate_git_hash_raw("tree", &[]);
        assert_eq!(empty_tree_hash, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    }

    #[test]
    fn test_sha1_basic() {
        // Test the basic SHA-1 implementation
        let result = sha1_hex(b"abc");
        assert_eq!(result, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn test_sha1_consistency() {
        // Test that our hash function is consistent
        let data = b"test data for consistency check";
        let hash1 = sha1_hash(data);
        let hash2 = sha1_hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_git_hash_format() {
        // Test that git hashes include the proper header
        let content = b"test content";
        let hash = calculate_git_hash_raw("blob", content);

        // Verify this produces the same result as manual calculation
        let manual_input = format!("blob {}\0test content", content.len());
        let manual_hash = sha1_hex(manual_input.as_bytes());

        assert_eq!(hash, manual_hash);
    }
}
