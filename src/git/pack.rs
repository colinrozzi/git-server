use super::objects::GitObject;
use super::repository::{GitRepoState, serialize_tree_object, serialize_commit_object};
use crate::utils::hash::sha1_hash;
use crate::utils::compression::compress_zlib;
use crate::utils::logging::safe_log as log;
use std::collections::HashSet;

/// Generate a Git pack file containing the specified objects and their dependencies
pub fn generate_pack_file(repo_state: &GitRepoState, object_hashes: &[String]) -> Vec<u8> {
    log("Generating pack file");
    
    // Collect only the requested objects and their dependencies
    let mut objects_to_include = HashSet::new();
    
    // Add requested objects and their dependencies
    for hash in object_hashes {
        add_object_with_dependencies(repo_state, hash, &mut objects_to_include);
    }
    
    // If no specific objects requested, include all objects
    if objects_to_include.is_empty() {
        for hash in repo_state.objects.keys() {
            objects_to_include.insert(hash.clone());
        }
    }
    
    let mut pack_data = Vec::new();
    
    // Pack file header: "PACK" + version (2) + number of objects
    pack_data.extend(b"PACK");
    pack_data.extend(&2u32.to_be_bytes()); // Version 2
    pack_data.extend(&(objects_to_include.len() as u32).to_be_bytes());
    
    // Add objects to pack
    for hash in &objects_to_include {
        if let Some(obj) = repo_state.objects.get(hash) {
            let (obj_type, obj_data) = match obj {
                GitObject::Blob { content } => (1u8, content.clone()), // OBJ_BLOB = 1
                GitObject::Tree { entries } => (2u8, serialize_tree_object(entries)), // OBJ_TREE = 2
                GitObject::Commit { tree, parents, author, committer, message } => {
                    // CRITICAL FIX: Use the EXACT same author/committer as when hash was calculated
                    let commit_data = serialize_commit_object(tree, parents, author, committer, message);
                    (3u8, commit_data) // OBJ_COMMIT = 3
                }
                GitObject::Tag { .. } => (4u8, vec![]), // OBJ_TAG = 4, not implemented
            };
            
            // Object header: type and size
            let size = obj_data.len();
            let mut header = vec![];
            
            // Git pack object header format
            // First byte: MTTT SSSS where M=more-size-bytes, TTT=type, SSSS=size-bits
            let mut size_to_encode = size;
            let first_byte = (obj_type << 4) | ((size_to_encode & 0x0F) as u8);
            size_to_encode >>= 4;
            
            if size_to_encode == 0 {
                // Size fits in 4 bits, no continuation needed
                header.push(first_byte);
            } else {
                // Size needs continuation bytes
                header.push(first_byte | 0x80); // Set continuation bit
                
                // Add continuation bytes
                while size_to_encode > 0 {
                    let mut byte = (size_to_encode & 0x7F) as u8;
                    size_to_encode >>= 7;
                    if size_to_encode > 0 {
                        byte |= 0x80; // Set continuation bit if more bytes follow
                    }
                    header.push(byte);
                }
            }
            
            pack_data.extend(header);
            
            // Compress object data with zlib (Git requirement)
            let compressed_data = compress_zlib(&obj_data);
            pack_data.extend(compressed_data);
            
            log(&format!("Added object {} ({} bytes)", hash, size));
        }
    }
    
    // Add SHA-1 checksum of pack file content (everything before the checksum)
    let pack_checksum = calculate_pack_sha1_checksum(&pack_data);
    pack_data.extend(&pack_checksum);
    
    log(&format!("Generated pack file: {} bytes", pack_data.len()));
    pack_data
}

/// Generate an empty pack file
pub fn generate_empty_pack() -> Vec<u8> {
    let mut pack_data = Vec::new();
    
    // Empty pack: "PACK" + version (2) + 0 objects + checksum
    pack_data.extend(b"PACK");
    pack_data.extend(&2u32.to_be_bytes()); // Version 2
    pack_data.extend(&0u32.to_be_bytes()); // 0 objects
    
    // Add proper checksum for empty pack
    let pack_checksum = calculate_pack_sha1_checksum(&pack_data);
    pack_data.extend(&pack_checksum);
    
    pack_data
}

/// Helper function to add an object and all its dependencies to the set
pub fn add_object_with_dependencies(repo_state: &GitRepoState, hash: &str, objects: &mut HashSet<String>) {
    if objects.contains(hash) {
        return; // Already processed
    }
    
    // Validate hash format
    if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        log(&format!("Warning: Invalid hash format: {}", hash));
        return;
    }
    
    if let Some(obj) = repo_state.objects.get(hash) {
        objects.insert(hash.to_string());
        log(&format!("Added object {} to pack", hash));
        
        match obj {
            GitObject::Commit { tree, parents, .. } => {
                // Validate tree exists
                if !repo_state.objects.contains_key(tree) {
                    log(&format!("Warning: Commit {} references missing tree {}", hash, tree));
                } else {
                    add_object_with_dependencies(repo_state, tree, objects);
                }
                
                // Validate parents exist  
                for parent in parents {
                    if !repo_state.objects.contains_key(parent) {
                        log(&format!("Warning: Commit {} references missing parent {}", hash, parent));
                    } else {
                        add_object_with_dependencies(repo_state, parent, objects);
                    }
                }
            }
            GitObject::Tree { entries } => {
                // Validate all tree entries exist
                for entry in entries {
                    if !repo_state.objects.contains_key(&entry.hash) {
                        log(&format!("Warning: Tree {} references missing object {}", hash, entry.hash));
                    } else {
                        add_object_with_dependencies(repo_state, &entry.hash, objects);
                    }
                }
            }
            GitObject::Tag { object, .. } => {
                if !repo_state.objects.contains_key(object) {
                    log(&format!("Warning: Tag {} references missing object {}", hash, object));
                } else {
                    add_object_with_dependencies(repo_state, object, objects);
                }
            }
            GitObject::Blob { .. } => {
                // Blobs don't reference other objects
            }
        }
    } else {
        log(&format!("Warning: Object not found in repository: {}", hash));
    }
}

/// Calculate SHA-1 checksum of the pack file content
fn calculate_pack_sha1_checksum(pack_data: &[u8]) -> [u8; 20] {
    // Calculate SHA-1 checksum of the pack file content
    // This should be the SHA-1 of everything before the checksum itself
    sha1_hash(pack_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::objects::TreeEntry;

    #[test]
    fn test_empty_pack_generation() {
        let empty_pack = generate_empty_pack();
        
        // Should start with "PACK"
        assert_eq!(&empty_pack[0..4], b"PACK");
        
        // Should have version 2
        let version = u32::from_be_bytes([empty_pack[4], empty_pack[5], empty_pack[6], empty_pack[7]]);
        assert_eq!(version, 2);
        
        // Should have 0 objects
        let object_count = u32::from_be_bytes([empty_pack[8], empty_pack[9], empty_pack[10], empty_pack[11]]);
        assert_eq!(object_count, 0);
        
        // Should end with 20-byte SHA-1 checksum
        assert_eq!(empty_pack.len(), 12 + 20); // header + checksum
    }

    #[test]
    fn test_dependency_tracking() {
        let mut repo = GitRepoState::new("test".to_string());
        
        // Use valid 40-character hex hashes for testing
        let blob_hash = "a94a8fe5ccb19ba61c4c0873d391e987982fbbd3"; // "test" blob
        let tree_hash = "b52168be5ea341e918a9cbbb76e28b85e36c5426"; // tree containing blob
        let commit_hash = "c3d8bb8ab1e38c5b2a0e57d0e33e91876e4f1b2f"; // commit
        
        // Create a blob
        let blob = GitObject::Blob { content: b"test".to_vec() };
        repo.add_object(blob_hash.to_string(), blob);
        
        // Create a tree referencing the blob
        let tree = GitObject::Tree {
            entries: vec![TreeEntry::blob("file.txt".to_string(), blob_hash.to_string())],
        };
        repo.add_object(tree_hash.to_string(), tree);
        
        // Create a commit referencing the tree
        let commit = GitObject::Commit {
            tree: tree_hash.to_string(),
            parents: vec![],
            author: "Test <test@example.com>".to_string(),
            committer: "Test <test@example.com>".to_string(),
            message: "Test commit".to_string(),
        };
        repo.add_object(commit_hash.to_string(), commit);
        
        // Track dependencies starting from the commit
        let mut objects = HashSet::new();
        add_object_with_dependencies(&repo, commit_hash, &mut objects);
        
        // Should include all three objects
        assert_eq!(objects.len(), 3);
        assert!(objects.contains(commit_hash));
        assert!(objects.contains(tree_hash));
        assert!(objects.contains(blob_hash));
    }

    #[test]
    fn test_pack_file_structure() {
        let mut repo = GitRepoState::new("test".to_string());
        
        // Add a simple blob with valid hash
        let blob_hash = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"; // "hello" blob
        let blob = GitObject::Blob { content: b"hello".to_vec() };
        repo.add_object(blob_hash.to_string(), blob);
        
        let pack = generate_pack_file(&repo, &[blob_hash.to_string()]);
        
        // Should start with pack header
        assert_eq!(&pack[0..4], b"PACK");
        
        // Should have version 2
        let version = u32::from_be_bytes([pack[4], pack[5], pack[6], pack[7]]);
        assert_eq!(version, 2);
        
        // Should have 1 object
        let object_count = u32::from_be_bytes([pack[8], pack[9], pack[10], pack[11]]);
        assert_eq!(object_count, 1);
        
        // Should end with 20-byte SHA-1 checksum
        assert!(pack.len() >= 32); // At minimum: 12 byte header + some object data + 20 byte checksum
    }
}
