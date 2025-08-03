use super::objects::{GitObject, TreeEntry};
use super::pack::parse_pack_file;
use crate::utils::hash::calculate_git_hash_raw;
use crate::utils::logging::safe_log as log;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GitRepoState {
    pub repo_name: String,

    // Git references (branches/tags) -> commit hash
    pub refs: HashMap<String, String>,

    // Git objects: hash -> object data
    pub objects: HashMap<String, GitObject>,

    // HEAD reference (usually "refs/heads/main")
    pub head: String,
}

impl Default for GitRepoState {
    fn default() -> Self {
        // Start with completely empty repository
        Self {
            repo_name: "git-server".to_string(),
            refs: HashMap::new(),    // No refs initially
            objects: HashMap::new(), // No objects initially
            head: "refs/heads/main".to_string(),
        }
    }
}

impl GitRepoState {
    /// Create a new repository with the given name
    pub fn new(name: String) -> Self {
        let mut repo = Self::default();
        repo.repo_name = name;
        repo
    }

    /// Get an object from the repository
    pub fn get_object(&self, hash: &str) -> Option<&GitObject> {
        self.objects.get(hash)
    }

    /// Get the commit hash for a reference
    pub fn get_ref(&self, ref_name: &str) -> Option<&String> {
        self.refs.get(ref_name)
    }

    /// Create a minimal repository with basic objects for testing
    pub fn ensure_minimal_objects(&mut self) {
        if self.objects.is_empty() {
            log("Creating minimal repository objects");
            self.create_initial_commit();
        }
    }

    /// Create an initial commit with a README file
    fn create_initial_commit(&mut self) {
        // Run SHA-1 diagnostic tests first
        self.test_sha1_against_git();

        // Create a simple blob (README file)
        let readme_content = b"# Git Server\n\nThis is a WebAssembly git server!\n";
        log(&format!(
            "README content: {:?}",
            String::from_utf8_lossy(readme_content)
        ));

        let readme_hash = calculate_git_hash_raw("blob", readme_content);
        self.add_object(
            readme_hash.clone(),
            GitObject::Blob {
                content: readme_content.to_vec(),
            },
        );
        log(&format!(
            "Created blob: {} (content hash should match)",
            readme_hash
        ));

        // Create a tree containing the README
        let tree_entries = vec![TreeEntry::blob(
            "README.md".to_string(),
            readme_hash.clone(),
        )];

        let tree_content = serialize_tree_object(&tree_entries);
        log(&format!("Tree content bytes: {:?}", tree_content));

        let tree_hash = calculate_git_hash_raw("tree", &tree_content);
        self.add_object(
            tree_hash.clone(),
            GitObject::Tree {
                entries: tree_entries,
            },
        );
        log(&format!(
            "Created tree: {} (should reference blob {})",
            tree_hash, readme_hash
        ));

        // Create a commit with exact Git format
        let author = "Git Server <git-server@example.com>";
        let commit_content_raw =
            serialize_commit_object(&tree_hash, &[], author, author, "Initial commit");

        log(&format!(
            "Commit content: {:?}",
            String::from_utf8_lossy(&commit_content_raw)
        ));

        let commit_hash = calculate_git_hash_raw("commit", &commit_content_raw);
        self.add_object(
            commit_hash.clone(),
            GitObject::Commit {
                tree: tree_hash.clone(),
                parents: vec![],
                author: author.to_string(),
                committer: author.to_string(),
                message: "Initial commit".to_string(),
            },
        );

        // Update refs
        self.update_ref("refs/heads/main".to_string(), commit_hash.clone());

        log(&format!("=== FINAL OBJECT HASHES ==="));
        log(&format!("Blob (README.md): {}", readme_hash));
        log(&format!("Tree (root): {}", tree_hash));
        log(&format!("Commit (main): {}", commit_hash));
        log(&format!("Refs: {:?}", self.refs));

        // Verify object references
        self.validate();

        log(&format!(
            "Created {} objects with proper SHA-1 hashes",
            self.objects.len()
        ));
    }

    /// SHA-1 diagnostic test function
    fn test_sha1_against_git(&self) {
        log("=== SHA-1 DIAGNOSTIC TEST ===");

        // Test 1: Simple blob
        let blob_content = b"hello world";
        let blob_hash = calculate_git_hash_raw("blob", blob_content);
        // Should match: echo "hello world" | git hash-object --stdin
        // Expected: 95d09f2b10159347eece71399a7e2e907ea3df4f
        log(&format!(
            "Blob 'hello world': {} (expect: 95d09f2b10159347eece71399a7e2e907ea3df4f)",
            blob_hash
        ));

        // Test 2: Empty tree
        let empty_tree_hash = calculate_git_hash_raw("tree", &[]);
        // Expected: 4b825dc642cb6eb9a060e54bf8d69288fbee4904
        log(&format!(
            "Empty tree: {} (expect: 4b825dc642cb6eb9a060e54bf8d69288fbee4904)",
            empty_tree_hash
        ));

        log("=== END SHA-1 DIAGNOSTIC ===");

        // Add detailed commit object debugging
        self.debug_commit_object_format();
    }

    /// Debug the exact commit object format
    fn debug_commit_object_format(&self) {
        log("=== COMMIT OBJECT DEBUG ===");

        // Recreate the exact same objects
        let tree_hash = "b0841aa3ac9b0dbe7aee598869498290a5a74a01";
        let author = "Git Server <git-server@example.com>";
        let timestamp = "1609459200 +0000";

        // Create commit content exactly as we do
        let mut commit_content = String::new();
        commit_content.push_str(&format!("tree {}\n", tree_hash));
        commit_content.push_str(&format!("author {} {}\n", author, timestamp));
        commit_content.push_str(&format!("committer {} {}\n", author, timestamp));
        commit_content.push('\n');
        commit_content.push_str("Initial commit");

        log(&format!("Commit content string: '{}'", commit_content));
        log(&format!(
            "Commit content bytes: {:?}",
            commit_content.as_bytes()
        ));
        log(&format!("Commit content length: {}", commit_content.len()));

        // Calculate hash
        let commit_hash = calculate_git_hash_raw("commit", commit_content.as_bytes());
        log(&format!("Our calculated hash: {}", commit_hash));

        // Create the Git header that goes into the hash calculation
        let header = format!("commit {}\0", commit_content.len());
        log(&format!("Git object header: '{}'", header));
        log(&format!(
            "Full hash input: header + content = {} + {}",
            header.len(),
            commit_content.len()
        ));

        // Show the exact bytes that get hashed
        let mut full_hash_input: Vec<u8> = Vec::new();
        full_hash_input.extend(header.as_bytes());
        full_hash_input.extend(commit_content.as_bytes());
        log(&format!("Full SHA-1 input bytes: {:?}", full_hash_input));

        log("=== END COMMIT OBJECT DEBUG ===");
    }

    /// Validate repository object references
    pub fn validate(&self) -> Vec<String> {
        log("=== VERIFYING OBJECT REFERENCES ===");
        let mut errors = Vec::new();

        // Check that all refs point to existing objects
        for (ref_name, hash) in &self.refs {
            if !self.objects.contains_key(hash) {
                errors.push(format!(
                    "Ref {} points to missing object {}",
                    ref_name, hash
                ));
            }
        }

        // Check that all object references are valid
        for (hash, obj) in &self.objects {
            match obj {
                GitObject::Commit { tree, parents, .. } => {
                    if !self.objects.contains_key(tree) {
                        errors.push(format!("Commit {} references missing tree {}", hash, tree));
                    } else {
                        log(&format!(
                            "✅ Commit {} references valid tree {}",
                            hash, tree
                        ));
                    }
                    for parent in parents {
                        if !self.objects.contains_key(parent) {
                            errors.push(format!(
                                "Commit {} references missing parent {}",
                                hash, parent
                            ));
                        }
                    }
                }
                GitObject::Tree { entries } => {
                    for entry in entries {
                        if !self.objects.contains_key(&entry.hash) {
                            errors.push(format!(
                                "Tree {} entry '{}' references missing object {}",
                                hash, entry.name, entry.hash
                            ));
                        } else {
                            log(&format!(
                                "✅ Tree entry '{}' references valid object {}",
                                entry.name, entry.hash
                            ));
                        }
                    }
                }
                GitObject::Tag { object, .. } => {
                    if !self.objects.contains_key(object) {
                        errors.push(format!("Tag {} references missing object {}", hash, object));
                    }
                }
                GitObject::Blob { .. } => {
                    // Blobs don't reference other objects
                }
            }
        }

        if errors.is_empty() {
            log("✅ Repository state validation passed");
        } else {
            log(&format!(
                "❌ Repository state has {} validation errors",
                errors.len()
            ));
            for error in &errors {
                log(&format!("  {}", error));
            }
        }
        log("=== END VERIFICATION ===");

        errors
    }

    /// Enhanced debugging for object validation issues
    pub fn debug_object_consistency(&self) {
        log("=== DETAILED OBJECT CONSISTENCY CHECK ===");

        for (stored_hash, obj) in &self.objects {
            log(&format!("Checking object: {}", stored_hash));

            // Re-serialize the object and recalculate its hash
            let (recalculated_hash, serialized_content) = match obj {
                GitObject::Blob { content } => {
                    let hash = calculate_git_hash_raw("blob", content);
                    (hash, content.clone())
                }
                GitObject::Tree { entries } => {
                    let serialized = serialize_tree_object(entries);
                    let hash = calculate_git_hash_raw("tree", &serialized);
                    (hash, serialized)
                }
                GitObject::Commit {
                    tree,
                    parents,
                    author,
                    committer,
                    message,
                } => {
                    let serialized =
                        serialize_commit_object(tree, parents, author, committer, message);
                    let hash = calculate_git_hash_raw("commit", &serialized);
                    (hash, serialized)
                }
                GitObject::Tag { .. } => {
                    // Tags not implemented yet
                    continue;
                }
            };

            if stored_hash != &recalculated_hash {
                log(&format!("❌ HASH MISMATCH for object {}", stored_hash));
                log(&format!("   Stored hash:      {}", stored_hash));
                log(&format!("   Recalculated:     {}", recalculated_hash));
                log(&format!("   Object type:      {}", obj.object_type()));
                log(&format!(
                    "   Serialized size:  {} bytes",
                    serialized_content.len()
                ));

                // Show first 100 bytes of serialized content for debugging
                let preview = if serialized_content.len() > 100 {
                    format!("{:?}...", &serialized_content[..100])
                } else {
                    format!("{:?}", serialized_content)
                };
                log(&format!("   Serialized preview: {}", preview));

                // For commits, show the exact string format
                if let GitObject::Commit { .. } = obj {
                    if let Ok(commit_str) = String::from_utf8(serialized_content.clone()) {
                        log(&format!("   Commit string: '{}'", commit_str));
                    }
                }
            } else {
                log(&format!(
                    "✅ Hash consistent for {} ({})",
                    obj.object_type(),
                    stored_hash
                ));
            }
        }

        log("=== END OBJECT CONSISTENCY CHECK ===");
    }

    /// Enhanced version of ensure_minimal_objects with detailed debugging
    pub fn ensure_minimal_objects_debug(&mut self) {
        if !self.objects.is_empty() {
            log("Objects already exist, running consistency checks...");
            self.debug_object_consistency();
            return;
        }

        log("Creating minimal repository objects with enhanced debugging");

        // Test our hash implementation first
        self.test_sha1_against_git();

        // Create objects with detailed logging
        log("=== CREATING REPOSITORY OBJECTS ===");

        // 1. Create README blob
        let readme_content = b"# Git Server\n\nThis is a WebAssembly git server!\n";
        log(&format!(
            "Creating README blob with {} bytes",
            readme_content.len()
        ));
        log(&format!(
            "README content: {:?}",
            String::from_utf8_lossy(readme_content)
        ));

        let readme_hash = calculate_git_hash_raw("blob", readme_content);
        let readme_blob = GitObject::Blob {
            content: readme_content.to_vec(),
        };
        self.add_object(readme_hash.clone(), readme_blob);

        // 2. Create tree
        let tree_entries = vec![TreeEntry::blob(
            "README.md".to_string(),
            readme_hash.clone(),
        )];
        let tree_content = serialize_tree_object(&tree_entries);
        log(&format!(
            "Creating tree with {} entries, {} bytes",
            tree_entries.len(),
            tree_content.len()
        ));
        log(&format!("Tree entries: {:?}", tree_entries));
        log(&format!("Tree content bytes: {:?}", tree_content));

        let tree_hash = calculate_git_hash_raw("tree", &tree_content);
        let tree_obj = GitObject::Tree {
            entries: tree_entries,
        };
        self.add_object(tree_hash.clone(), tree_obj);

        // 3. Create commit
        let author = "Git Server <git-server@example.com>";
        let commit_content =
            serialize_commit_object(&tree_hash, &[], author, author, "Initial commit");
        log(&format!(
            "Creating commit with {} bytes",
            commit_content.len()
        ));
        log(&format!(
            "Commit content: {:?}",
            String::from_utf8_lossy(&commit_content)
        ));

        let commit_hash = calculate_git_hash_raw("commit", &commit_content);
        let commit_obj = GitObject::Commit {
            tree: tree_hash.clone(),
            parents: vec![],
            author: author.to_string(),
            committer: author.to_string(),
            message: "Initial commit".to_string(),
        };
        self.add_object(commit_hash.clone(), commit_obj);

        // 4. Update refs
        self.update_ref("refs/heads/main".to_string(), commit_hash.clone());

        log("=== FINAL VERIFICATION ===");
        log(&format!("Created {} objects:", self.objects.len()));
        log(&format!("  Blob (README.md): {}", readme_hash));
        log(&format!("  Tree (root):      {}", tree_hash));
        log(&format!("  Commit (main):    {}", commit_hash));
        log(&format!("Refs: {:?}", self.refs));

        // Run all consistency checks
        self.debug_object_consistency();
        self.validate();

        log(&format!(
            "Repository initialization complete with {} objects",
            self.objects.len()
        ));
    }

    /// Add an object to the repository (Smart HTTP)
    pub fn add_object(&mut self, hash: String, object: GitObject) {
        log(&format!(
            "Adding object to repository: {} ({})",
            hash,
            match object {
                GitObject::Blob { .. } => "blob",
                GitObject::Tree { .. } => "tree",
                GitObject::Commit { .. } => "commit",
                GitObject::Tag { .. } => "tag",
            }
        ));
        self.objects.insert(hash, object);
    }

    /// Update or create a reference
    pub fn update_ref(&mut self, ref_name: String, new_hash: String) {
        log(&format!("Updating ref {} to {}", ref_name, new_hash));
        self.refs.insert(ref_name, new_hash);
    }

    /// Delete a reference
    pub fn delete_ref(&mut self, ref_name: &str) -> Option<String> {
        log(&format!("Deleting ref {}", ref_name));
        self.refs.remove(ref_name)
    }

    /// Check if an object exists in the repository
    pub fn has_object(&self, hash: &str) -> bool {
        self.objects.contains_key(hash)
    }

    /// Get current value of a reference
    pub fn get_ref_value(&self, ref_name: &str) -> Option<&String> {
        self.refs.get(ref_name)
    }

    /// Repository update methods for push operations
    pub fn process_pack_file(&mut self, pack_data: &[u8]) -> Result<Vec<String>, String> {
        log("Processing incoming pack file for repository updates");

        log(&format!("pack file {:?}", pack_data));
        let objects = parse_pack_file(pack_data)?;
        let mut new_hashes = Vec::new();

        log(&format!("Parsed {} objects from pack file", objects.len()));

        for obj in objects {
            log(&format!(
                "Processing object: {} ({})",
                obj.object_type(),
                match &obj {
                    GitObject::Blob { .. } => "blob",
                    GitObject::Tree { .. } => "tree",
                    GitObject::Commit { .. } => "commit",
                    GitObject::Tag { .. } => "tag",
                }
            ));
            // DEBUG: Log raw object details for debugging
            match &obj {
                GitObject::Commit { tree, parents, author, committer, message } => {
                    log(&format!("COMMIT DEBUG - tree: {}, parents: {:?}, author: {}, committer: {}, message: {}", tree, parents, author, committer, message));
                    
                    // DEBUG: Log the exact serialized content
                    let serialized = crate::git::repository::serialize_commit_object(tree, parents, author, committer, message);
                    log(&format!("COMMIT SERIALIZED: {:?}", std::str::from_utf8(&serialized).unwrap_or("<invalid utf8>")));
                    log(&format!("COMMIT SERIALIZED BYTES: {:?}", serialized));
                }
                _ => {}
            }
            
            let hash = crate::utils::hash::calculate_git_hash(&obj);
            log(&format!("Calculated hash: {}", hash));

            // Only add new objects, skip duplicates
            if !self.objects.contains_key(&hash) {
                log(&format!("Adding new object with hash: {}", hash));
                self.add_object(hash.clone(), obj);
                new_hashes.push(hash);
            } else {
                log(&format!(
                    "Object with hash {} already exists, skipping",
                    hash
                ));
            }
        }

        log(&format!(
            "Added {} new objects to repository",
            new_hashes.len()
        ));
        
        // DEBUG: Log all calculated hashes for comparison
        log(&format!("All calculated hashes: {:?}", new_hashes));
        Ok(new_hashes)
    }

    /// Update repository refs based on push commands
    pub fn update_refs_from_push(
        &mut self,
        ref_updates: Vec<(String, String, String)>,
    ) -> Result<Vec<String>, String> {
        let mut updated_refs = Vec::new();

        for (ref_name, old_oid, new_oid) in ref_updates {
            log(&format!(
                "Processing ref update: {} {} -> {}",
                ref_name, old_oid, new_oid
            ));

            // Validate new OID exists
            if !self.objects.contains_key(&new_oid) {
                return Err(format!(
                    "Cannot update ref {}: new object {} not found in repository",
                    ref_name, new_oid
                ));
            }

            // Handle different types of ref updates
            if old_oid == "0000000000000000000000000000000000000000" {
                // Create new reference
                log(&format!("Creating new reference {}", ref_name));
                self.update_ref(ref_name.clone(), new_oid);
                updated_refs.push(format!("create {}", ref_name));

                // For first branch created to empty repo, set HEAD
                if self.refs.len() == 1 && ref_name.starts_with("refs/heads/") {
                    self.head = ref_name.clone();
                    log(&format!("Setting HEAD to {}", ref_name));
                }
            } else if new_oid == "0000000000000000000000000000000000000000" {
                // Delete reference (not in scope for empty repo push)
                log(&format!("Deleting reference {}", ref_name));
                self.delete_ref(&ref_name);
                updated_refs.push(format!("delete {}", ref_name));
            } else {
                // Update existing reference (not in scope for empty repo push)
                log(&format!("Updating existing reference {}", ref_name));
                self.update_ref(ref_name.clone(), new_oid);
                updated_refs.push(format!("update {}", ref_name));
            }
        }

        log(&format!(
            "Updated {} refs in repository",
            updated_refs.len()
        ));
        Ok(updated_refs)
    }

    /// Process a complete push operation
    pub fn process_push_operation(
        &mut self,
        pack_data: &[u8],
        ref_updates: Vec<(String, String, String)>,
    ) -> Result<Vec<String>, String> {
        log("Processing complete push operation");

        // Phase 1: Parse and store pack file objects
        let new_hashes = self.process_pack_file(pack_data)?;

        log(&format!(
            "Processed {} new objects from pack file",
            new_hashes.len()
        ));

        // Phase 2: Validate all ref targets exist
        for (ref_name, old_oid, new_oid) in &ref_updates {
            log(&format!("Validating ref update: {} {} -> {}", ref_name, old_oid, new_oid));
            log(&format!("Available objects: {:?}", self.objects.keys().collect::<Vec<_>>()));
            if !self.objects.contains_key(new_oid) {
                return Err(format!(
                    "Ref update validation failed: object {} not found",
                    new_oid
                ));
            }
        }

        log("All ref updates validated successfully");

        // Phase 3: Update refs
        let updated_refs = self.update_refs_from_push(ref_updates)?;

        log(&format!(
            "Updated {} references in repository",
            updated_refs.len()
        ));
        // Phase 4: Verify repository consistency
        let errors = self.validate();
        if !errors.is_empty() {
            return Err(format!("Repository validation failed: {:?}", errors));
        }

        log(&format!("Push operation completed successfully"));
        Ok(updated_refs)
    }

    /// Ensure we have minimal objects for Smart HTTP testing
    pub fn ensure_minimal_objects_for_smart_http(&mut self) {
        if self.objects.is_empty() {
            log("Creating minimal objects for Smart HTTP testing");

            // Create a simple blob
            let blob = GitObject::Blob {
                content: b"Hello, Smart HTTP Git Server!\n".to_vec(),
            };
            let blob_hash = crate::utils::hash::calculate_git_hash(&blob);
            self.add_object(blob_hash.clone(), blob);

            // Create a simple tree containing the blob
            let tree_entry = TreeEntry::new(
                "100644".to_string(),
                "README.md".to_string(),
                blob_hash.clone(),
            );
            let tree = GitObject::Tree {
                entries: vec![tree_entry],
            };
            let tree_hash = crate::utils::hash::calculate_git_hash(&tree);
            self.add_object(tree_hash.clone(), tree);

            // Create an initial commit
            let commit = GitObject::Commit {
                tree: tree_hash,
                parents: vec![], // No parents - initial commit
                author: "Git Server <git@server.com> 1640995200 +0000".to_string(),
                committer: "Git Server <git@server.com> 1640995200 +0000".to_string(),
                message: "Initial commit\n\nCreated by Smart HTTP Git Server".to_string(),
            };
            let commit_hash = crate::utils::hash::calculate_git_hash(&commit);
            self.add_object(commit_hash.clone(), commit);

            // Create main branch pointing to the commit
            self.update_ref("refs/heads/main".to_string(), commit_hash);
            self.head = "refs/heads/main".to_string();

            log(&format!(
                "Created {} objects and 1 ref for testing",
                self.objects.len()
            ));
        }
    }
}

/// Serialize a tree object to Git's binary format
pub fn serialize_tree_object(entries: &[TreeEntry]) -> Vec<u8> {
    let mut data = Vec::new();

    // Sort entries by name (Git requirement for consistent hashing)
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by(|a, b| a.name.cmp(&b.name));

    for entry in &sorted_entries {
        // Mode as string (no leading zeros for 100644)
        data.extend(entry.mode.as_bytes());
        data.push(b' '); // Space separator

        // Filename
        data.extend(entry.name.as_bytes());
        data.push(0); // Null terminator

        // Hash as 20 binary bytes (not hex string)
        if entry.hash.len() == 40 {
            for i in (0..40).step_by(2) {
                if let Ok(byte) = u8::from_str_radix(&entry.hash[i..i + 2], 16) {
                    data.push(byte);
                } else {
                    // Handle invalid hex - this should not happen with proper hashes
                    log(&format!("Warning: invalid hex in hash {}", entry.hash));
                    break;
                }
            }
        } else {
            log(&format!(
                "Warning: invalid hash length for {}: {}",
                entry.name, entry.hash
            ));
        }
    }

    data
}

/// Serialize a commit object to Git's text format
pub fn serialize_commit_object(
    tree: &str,
    parents: &[String],
    author: &str,
    committer: &str,
    message: &str,
) -> Vec<u8> {
    let mut content = String::new();

    content.push_str(&format!("tree {}\n", tree));

    for parent in parents {
        content.push_str(&format!("parent {}\n", parent));
    }

    // Use the actual author and committer strings as-is (they include timestamps)
    content.push_str(&format!("author {}\n", author));
    content.push_str(&format!("committer {}\n", committer));
    content.push('\n'); // Empty line before message
    content.push_str(message);
    content.push('\n'); // Trailing newline after message

    // CRITICAL FIX: Git commit objects should NOT have trailing newlines
    // The message itself should be the final content without additional newlines

    content.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_creation() {
        let repo = GitRepoState::new("test-repo".to_string());
        assert_eq!(repo.repo_name, "test-repo");
        assert_eq!(repo.head, "refs/heads/main");
        assert!(repo.objects.is_empty());
    }

    #[test]
    fn test_object_management() {
        let mut repo = GitRepoState::new("test".to_string());

        let blob = GitObject::Blob {
            content: b"test content".to_vec(),
        };
        repo.add_object("abc123".to_string(), blob);

        assert!(repo.get_object("abc123").is_some());
        assert!(repo.get_object("nonexistent").is_none());
    }

    #[test]
    fn test_ref_management() {
        let mut repo = GitRepoState::new("test".to_string());

        repo.update_ref("refs/heads/feature".to_string(), "def456".to_string());
        assert_eq!(
            repo.get_ref("refs/heads/feature"),
            Some(&"def456".to_string())
        );
        assert_eq!(repo.get_ref("refs/heads/nonexistent"), None);
    }

    #[test]
    fn test_tree_serialization() {
        let entries = vec![
            TreeEntry::blob(
                "file.txt".to_string(),
                "abc123def456789012345678901234567890abcd".to_string(),
            ),
            TreeEntry::tree(
                "dir".to_string(),
                "def456789012345678901234567890abcdef1234".to_string(),
            ),
        ];

        let serialized = serialize_tree_object(&entries);

        // Should contain modes, names, and binary hashes
        assert!(!serialized.is_empty());
        // Should contain the filenames
        assert!(serialized.windows(8).any(|w| w == b"file.txt"));
        assert!(serialized.windows(3).any(|w| w == b"dir"));
    }
}
