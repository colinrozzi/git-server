use super::objects::{GitObject, TreeEntry};
use super::pack::parse_pack_file;
use crate::bindings::theater::simple::http_types::{HttpRequest, HttpResponse};
use crate::protocol::http::{
    create_error_response, create_response, create_status_response,
    create_status_response_with_capabilities, encode_flush_pkt, encode_pkt_line,
    encode_sideband_data, serialize_object_for_pack, CAPABILITIES, MAX_SIDEBAND_DATA,
};
use crate::protocol::version_one::{parse_receive_pack_request, PushRequest};
use crate::protocol::version_two::{parse_command_request, CommandRequest};
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
    /// Handle GET /info/refs - Support both Protocol v1 and v2
    pub fn handle_smart_info_refs(&mut self, service: &str) -> HttpResponse {
        log(&format!(
            "Processing info/refs request for service: {}",
            service
        ));

        match service {
            "git-upload-pack" => {
                // Upload-pack supports Protocol v2
                self.handle_upload_pack_info_refs()
            }
            "git-receive-pack" => {
                // Receive-pack falls back to Protocol v1 for compatibility
                self.handle_receive_pack_info_refs_v1()
            }
            _ => create_error_response("Unknown service"),
        }
    }

    /// Protocol v2 capability advertisement for upload-pack (fetch operations)
    fn handle_upload_pack_info_refs(&self) -> HttpResponse {
        log("Generating Protocol v2 capability advertisement for upload-pack");

        let mut response_data = Vec::new();

        // Protocol v2 format for upload-pack
        response_data.extend(encode_pkt_line(b"version 2\n"));
        response_data.extend(encode_pkt_line(b"agent=git-server/0.1.0\n"));
        response_data.extend(encode_pkt_line(b"object-format=sha1\n"));
        response_data.extend(encode_pkt_line(b"server-option\n"));
        response_data.extend(encode_pkt_line(b"ls-refs=symrefs peel ref-prefix unborn\n"));
        response_data.extend(encode_pkt_line(
            b"fetch=shallow thin-pack no-progress include-tag ofs-delta wait-for-done\n",
        ));
        response_data.extend(encode_pkt_line(b"object-info=size\n"));
        response_data.extend(encode_flush_pkt());

        create_response(
            200,
            "application/x-git-upload-pack-advertisement",
            &response_data,
        )
    }

    /// Protocol v1 capability advertisement for receive-pack (push operations)
    fn handle_receive_pack_info_refs_v1(&mut self) -> HttpResponse {
        log(
            "Generating Protocol v1 capability advertisement for receive-pack (push compatibility)",
        );

        let mut response_data = Vec::new();

        //
        // 1. Smart-HTTP banner
        //
        let banner = b"# service=git-receive-pack\n";
        response_data.extend(encode_pkt_line(banner));
        response_data.extend(encode_flush_pkt()); // flush-pkt after banner

        // Protocol v1 format - advertise refs first, then capabilities
        if self.refs.is_empty() {
            // Empty repository - advertise capabilities on the null ref
            let line = format!(
                "0000000000000000000000000000000000000000 capabilities^{{}}\0{}\n",
                CAPABILITIES
            );
            response_data.extend(encode_pkt_line(line.as_bytes()));
        } else {
            // Advertise existing refs with capabilities on the first ref
            let mut refs: Vec<_> = self.refs.iter().collect();
            refs.sort_by_key(|(name, _)| *name);

            let mut first_ref = true;
            for (ref_name, hash) in refs {
                if first_ref {
                    // First ref includes capabilities
                    let line = format!("{} {}\0{}\n", hash, ref_name, CAPABILITIES);
                    response_data.extend(encode_pkt_line(line.as_bytes()));
                    first_ref = false;
                } else {
                    let line = format!("{} {}\n", hash, ref_name);
                    response_data.extend(encode_pkt_line(line.as_bytes()));
                }
            }
        }

        response_data.extend(encode_flush_pkt());

        log("returning response");
        log(&String::from_utf8(response_data.clone()).unwrap());
        create_response(
            200,
            "application/x-git-receive-pack-advertisement",
            &response_data,
        )
    }

    pub fn handle_receive_pack_request(&mut self, request: &HttpRequest) -> HttpResponse {
        log("handle_receive_pack");

        let body = match &request.body {
            Some(b) => b,
            None => {
                log("missing request body, returning with a status response");
                return create_status_response(false, vec!["unpack missing-request".to_string()]);
            }
        };

        log("body found, parsing request");
        match parse_receive_pack_request(body) {
            Ok(push) => self.handle_v1_push(push),
            Err(e) => {
                // For parse errors, we don't have capabilities yet, so use basic response
                create_status_response(false, vec![format!("unpack {}", e)])
            }
        }
    }

    pub fn handle_upload_pack_request(&mut self, request: &HttpRequest) -> HttpResponse {
        log("handle_upload_pack");

        log("Processing Protocol v2 upload-pack request");
        let body = match &request.body {
            Some(b) => b,
            None => return create_error_response("Missing request body"),
        };

        let parsed = match parse_command_request(body) {
            Ok(req) => req,
            Err(e) => return create_error_response(&e),
        };

        match parsed.command.as_str() {
            "ls-refs" => self.handle_ls_refs(&parsed),
            "fetch" => self.handle_fetch(&parsed),
            "object-info" => self.handle_object_info(&parsed),
            _ => create_error_response(&format!("Unknown command: {}", parsed.command)),
        }
    }

    fn handle_v1_push(&mut self, push: PushRequest) -> HttpResponse {
        log("Processing Protocol v1 push operation");

        if push.ref_updates.is_empty() && push.pack_data.is_empty() {
            return create_status_response_with_capabilities(true, vec![], &push.capabilities);
        }

        match self.process_push_operation(&push.pack_data, push.ref_updates) {
            Ok(statuses) => {
                log("Push operation successful, processing statuses");
                let ref_statuses: Vec<String> = statuses
                    .iter()
                    .map(|status| {
                        if status.starts_with("create ") {
                            format!("ok {}", &status[7..])
                        } else if status.starts_with("update ") {
                            format!("ok {}", &status[7..])
                        } else {
                            status.clone()
                        }
                    })
                    .collect();
                create_status_response_with_capabilities(true, ref_statuses, &push.capabilities)
            }
            Err(e) => create_status_response_with_capabilities(
                false,
                vec![format!("unpack {}", e)],
                &push.capabilities,
            ),
        }
    }

    fn handle_ls_refs(&self, _request: &CommandRequest) -> HttpResponse {
        log("Handling ls-refs command");
        let mut response = Vec::new();

        if self.refs.is_empty() {
            log("Empty repository - showing unborn HEAD");
            response.extend(encode_pkt_line(
                "unborn HEAD symref-target:refs/heads/main\n".as_bytes(),
            ));
        } else {
            let mut refs: Vec<_> = self.refs.iter().collect();
            refs.sort_by_key(|(name, _)| *name);

            for (ref_name, hash) in refs {
                let line = format!("{} {}\n", hash, ref_name);
                response.extend(encode_pkt_line(line.as_bytes()));
            }
        }

        response.extend(encode_flush_pkt());
        create_response(200, "application/x-git-upload-pack-result", &response)
    }

    fn handle_fetch(&self, request: &CommandRequest) -> HttpResponse {
        log("Handling Protocol v2 fetch command");

        // Parse want lines from request args
        let mut wants = Vec::new();
        let mut has_done = false;

        for arg in &request.args {
            if arg.starts_with("want ") {
                wants.push(arg[5..].to_string()); // Remove "want " prefix
                log(&format!("Client wants: {}", &arg[5..]));
            } else if arg == "done" {
                has_done = true;
                log("Client sent 'done' - negotiation finished, skipping acknowledgments");
            }
        }

        if wants.is_empty() {
            log("Error: No wants specified in fetch request");
            return create_error_response("No wants specified");
        }

        log(&format!(
            "Fetch request: wants={}, done={}",
            wants.len(),
            has_done
        ));

        // Generate packfile for wanted objects
        match self.generate_packfile_for_wants(&wants) {
            Ok(packfile) => {
                log(&format!("Generated packfile: {} bytes", packfile.len()));

                let mut response = Vec::new();

                /* ----- 1.  acknowledgments  (only when !has_done) ----- */
                if !has_done {
                    response.extend(encode_pkt_line(b"acknowledgments\n"));
                    response.extend(encode_pkt_line(b"NAK\n")); // or real ACK/ready lines
                    response.extend(b"0001"); // delim-pkt -> next section
                }

                // Packfile section header
                response.extend(encode_pkt_line(b"packfile\n"));

                /* ----- 3.  side-band-encode the pack ----- */
                let mut pos = 0;
                // ----- 3. side-band-encode the pack -----
                while pos < packfile.len() {
                    // how much of the pack we can send in this side-band frame
                    let chunk_end = std::cmp::min(pos + MAX_SIDEBAND_DATA, packfile.len());

                    // helper builds: <4-byte length><0x01><payload>
                    response.extend(encode_sideband_data(1, &packfile[pos..chunk_end]));

                    pos = chunk_end;
                }

                // End packfile section with flush packet
                response.extend(encode_flush_pkt()); // 0000 - end of response

                log(&format!("Total response size: {} bytes", response.len()));
                create_response(200, "application/x-git-upload-pack-result", &response)
            }
            Err(e) => {
                log(&format!("Failed to generate packfile: {}", e));
                create_error_response(&format!("packfile generation failed: {}", e))
            }
        }
    }

    fn handle_object_info(&self, _request: &CommandRequest) -> HttpResponse {
        create_error_response("object-info not implemented yet")
    }

    fn generate_packfile_for_wants(&self, wants: &[String]) -> Result<Vec<u8>, String> {
        log(&format!("Generating packfile for {} wants", wants.len()));

        // Collect all objects needed for the wants
        let objects_to_send = self.collect_objects_for_wants(wants)?;
        log(&format!("Collected objects: {:?}", objects_to_send));

        // Generate the packfile
        self.generate_simple_packfile(&objects_to_send)
    }

    fn generate_simple_packfile(&self, object_ids: &[String]) -> Result<Vec<u8>, String> {
        use crate::utils::hash::sha1_hash;

        let mut pack = Vec::new();

        // Pack header: "PACK" + version(2) + object_count
        pack.extend(b"PACK");
        pack.extend(&2u32.to_be_bytes()); // version 2
        pack.extend(&(object_ids.len() as u32).to_be_bytes());

        log(&format!(
            "Pack header: version=2, objects={}",
            object_ids.len()
        ));

        // Add each object
        for obj_id in object_ids {
            if let Some(obj) = self.objects.get(obj_id) {
                log(&format!("Processing object: {}", obj));
                let obj_data = serialize_object_for_pack(obj)?;
                pack.extend(&obj_data);
            } else {
                return Err(format!("Object not found: {}", obj_id));
            }
        }

        // Pack checksum (SHA1 of entire pack so far)
        let checksum = sha1_hash(&pack);
        pack.extend(&checksum);

        log(&format!("Generated packfile: {:?}", pack));
        Ok(pack)
    }

    fn collect_objects_for_wants(&self, wants: &[String]) -> Result<Vec<String>, String> {
        use std::collections::HashSet;
        let mut objects = HashSet::new();

        for want_hash in wants {
            // Add the wanted object itself
            objects.insert(want_hash.clone());

            // If it's a commit, traverse to get tree + blobs
            if let Some(obj) = self.objects.get(want_hash) {
                match obj {
                    crate::git::objects::GitObject::Commit { tree, parents, .. } => {
                        // Add the tree
                        objects.insert(tree.clone());

                        // Add all objects in the tree
                        self.collect_tree_objects(tree, &mut objects)?;

                        // Add parent commits recursively
                        for parent_hash in parents {
                            self.collect_commit_ancestors(parent_hash, &mut objects)?;
                        }
                    }
                    crate::git::objects::GitObject::Tree { .. } => {
                        // If want is a tree, collect all its objects
                        self.collect_tree_objects(want_hash, &mut objects)?;
                    }
                    _ => {
                        // Blob or tag, just include it
                    }
                }
            } else {
                return Err(format!("Wanted object not found: {}", want_hash));
            }
        }

        Ok(objects.into_iter().collect())
    }

    fn collect_tree_objects(
        &self,
        tree_hash: &str,
        objects: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        if let Some(crate::git::objects::GitObject::Tree { entries }) = self.objects.get(tree_hash)
        {
            for entry in entries {
                objects.insert(entry.hash.clone());

                // If this entry is also a tree, recurse
                if entry.mode == "040000" {
                    // Directory mode
                    self.collect_tree_objects(&entry.hash, objects)?;
                }
            }
        }
        Ok(())
    }

    fn collect_commit_ancestors(
        &self,
        commit_hash: &str,
        objects: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        if objects.contains(commit_hash) {
            return Ok(()); // Already processed
        }

        objects.insert(commit_hash.to_string());

        if let Some(crate::git::objects::GitObject::Commit { tree, parents, .. }) =
            self.objects.get(commit_hash)
        {
            // Add the tree and its contents
            objects.insert(tree.clone());
            self.collect_tree_objects(tree, objects)?;

            // Recurse to parents
            for parent_hash in parents {
                self.collect_commit_ancestors(parent_hash, objects)?;
            }
        }

        Ok(())
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
                GitObject::Commit {
                    tree,
                    parents,
                    author,
                    committer,
                    message,
                } => {
                    log(&format!("COMMIT DEBUG - tree: {}, parents: {:?}, author: {}, committer: {}, message: {}", tree, parents, author, committer, message));

                    // DEBUG: Log the exact serialized content
                    let serialized = crate::git::repository::serialize_commit_object(
                        tree, parents, author, committer, message,
                    );
                    log(&format!(
                        "COMMIT SERIALIZED: {:?}",
                        std::str::from_utf8(&serialized).unwrap_or("<invalid utf8>")
                    ));
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
            log(&format!(
                "Validating ref update: {} {} -> {}",
                ref_name, old_oid, new_oid
            ));
            log(&format!(
                "Available objects: {:?}",
                self.objects.keys().collect::<Vec<_>>()
            ));
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
