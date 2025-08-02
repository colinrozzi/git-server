// Pack file implementation for Smart HTTP
// This handles the binary pack file format used by Git

use crate::git::objects::GitObject;
use crate::git::repository::GitRepoState;
use crate::utils::logging::safe_log as log;


/// Serialize an object for inclusion in a pack file
pub fn serialize_pack_object(object: &GitObject) -> Result<Vec<u8>, String> {
    let (obj_type_num, content) = match object {
        GitObject::Blob { content } => (1u8, content.clone()),
        GitObject::Tree { entries } => (2u8, serialize_tree_for_pack(entries)?),
        GitObject::Commit { tree, parents, author, committer, message } => {
            (3u8, serialize_commit_for_pack(tree, parents, author, committer, message)?)
        }
        GitObject::Tag { object, tag_type, tagger, message } => {
            (4u8, serialize_tag_for_pack(object, tag_type, tagger, message)?)
        }
    };
    
    let mut result = Vec::new();
    
    // Encode object header (type + size)
    encode_pack_object_header(&mut result, obj_type_num, content.len());
    
    // Compress content with zlib
    let compressed = crate::utils::compression::compress_zlib(&content);
    result.extend(compressed);
    
    Ok(result)
}

/// Encode pack object header (variable-length encoding)
fn encode_pack_object_header(output: &mut Vec<u8>, obj_type: u8, size: usize) {
    let mut remaining_size = size;
    
    // First byte: type (bits 4-6) + size (bits 0-3) + continuation bit (bit 7)
    let mut byte = (obj_type << 4) | (remaining_size as u8 & 0x0F);
    remaining_size >>= 4;
    
    // If there's more size data, set continuation bit
    if remaining_size > 0 {
        byte |= 0x80;
    }
    
    output.push(byte);
    
    // Continue with remaining size bytes
    while remaining_size > 0 {
        let mut byte = remaining_size as u8 & 0x7F;
        remaining_size >>= 7;
        
        if remaining_size > 0 {
            byte |= 0x80; // Set continuation bit
        }
        
        output.push(byte);
    }
}

/// Serialize tree object for pack format
fn serialize_tree_for_pack(entries: &[crate::git::objects::TreeEntry]) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    
    for entry in entries {
        // Format: "<mode> <name>\0<20-byte-hash>"
        result.extend(entry.mode.as_bytes());
        result.push(b' ');
        result.extend(entry.name.as_bytes());
        result.push(0); // Null separator
        
        // Convert hex hash to binary
        let hash_bytes = hex::decode(&entry.hash)
            .map_err(|_| format!("Invalid hash in tree entry: {}", entry.hash))?;
        if hash_bytes.len() != 20 {
            return Err(format!("Invalid hash length: {}", entry.hash));
        }
        result.extend(hash_bytes);
    }
    
    Ok(result)
}

/// Serialize commit object for pack format
fn serialize_commit_for_pack(
    tree: &str,
    parents: &[String],
    author: &str,
    committer: &str,
    message: &str,
) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    
    // Tree line
    result.extend(format!("tree {}\n", tree).as_bytes());
    
    // Parent lines
    for parent in parents {
        result.extend(format!("parent {}\n", parent).as_bytes());
    }
    
    // Author and committer
    result.extend(format!("author {}\n", author).as_bytes());
    result.extend(format!("committer {}\n", committer).as_bytes());
    
    // Empty line before message
    result.push(b'\n');
    
    // Commit message
    result.extend(message.as_bytes());
    
    Ok(result)
}

/// Serialize tag object for pack format
fn serialize_tag_for_pack(
    object: &str,
    tag_type: &str,
    tagger: &str,
    message: &str,
) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    
    result.extend(format!("object {}\n", object).as_bytes());
    result.extend(format!("type {}\n", tag_type).as_bytes());
    result.extend(format!("tag {}\n", tag_type).as_bytes()); // Tag name (simplified)
    
    if !tagger.is_empty() {
        result.extend(format!("tagger {}\n", tagger).as_bytes());
    }
    
    result.push(b'\n');
    result.extend(message.as_bytes());
    
    Ok(result)
}

/// Process pack data and add objects to repository
pub fn process_pack_data(repo_state: &mut GitRepoState, pack_data: &[u8]) -> Result<(), String> {
    log(&format!("Processing pack data: {} bytes", pack_data.len()));
    
    // Verify minimum pack size (header + checksum)
    if pack_data.len() < 32 {
        return Err("Pack data too short".to_string());
    }
    
    // Verify pack header
    if &pack_data[0..4] != b"PACK" {
        return Err("Invalid pack header".to_string());
    }
    
    let version = u32::from_be_bytes([pack_data[4], pack_data[5], pack_data[6], pack_data[7]]);
    if version != 2 {
        return Err(format!("Unsupported pack version: {}", version));
    }
    
    let object_count = u32::from_be_bytes([pack_data[8], pack_data[9], pack_data[10], pack_data[11]]);
    log(&format!("Pack contains {} objects", object_count));
    
    // Parse objects from pack
    let mut offset = 12;
    for i in 0..object_count {
        match parse_pack_object(pack_data, &mut offset) {
            Ok((hash, object)) => {
                log(&format!("Unpacked object {}: {} ({})", i + 1, hash, get_object_type_name(&object)));
                repo_state.add_object(hash, object);
            }
            Err(e) => {
                return Err(format!("Failed to parse object {}: {}", i + 1, e));
            }
        }
    }
    
    // Verify pack checksum
    if pack_data.len() < offset + 20 {
        return Err("Pack missing checksum".to_string());
    }
    
    let expected_checksum = &pack_data[pack_data.len() - 20..];
    let calculated_checksum = crate::utils::hash::sha1_hash(&pack_data[..pack_data.len() - 20]);
    
    if expected_checksum != calculated_checksum {
        return Err("Pack checksum mismatch".to_string());
    }
    
    log("Pack data processed successfully");
    Ok(())
}

/// Parse a single object from pack data
fn parse_pack_object(pack_data: &[u8], offset: &mut usize) -> Result<(String, GitObject), String> {
    // Parse object header (type and size)
    let (obj_type, uncompressed_size) = parse_pack_object_header(pack_data, offset)?;
    
    // Read and decompress object data
    let decompressed = decompress_pack_object_data(pack_data, offset, uncompressed_size)?;
    
    // Parse object based on type
    let object = match obj_type {
        1 => GitObject::Blob { content: decompressed },
        2 => parse_tree_from_pack(&decompressed)?,
        3 => parse_commit_from_pack(&decompressed)?,
        4 => parse_tag_from_pack(&decompressed)?,
        _ => return Err(format!("Unknown object type: {}", obj_type)),
    };
    
    // Calculate object hash for verification
    let hash = calculate_object_hash(&object);
    
    Ok((hash, object))
}

/// Parse pack object header and return (type, uncompressed_size)
fn parse_pack_object_header(pack_data: &[u8], offset: &mut usize) -> Result<(u8, usize), String> {
    if *offset >= pack_data.len() {
        return Err("Unexpected end of pack data".to_string());
    }
    
    let first_byte = pack_data[*offset];
    *offset += 1;
    
    let obj_type = (first_byte >> 4) & 0x07;
    let mut size = (first_byte & 0x0F) as usize;
    let mut shift = 4;
    
    // Variable-length size encoding
    let mut current_byte = first_byte;
    while current_byte & 0x80 != 0 {
        if *offset >= pack_data.len() {
            return Err("Unexpected end of pack data in header".to_string());
        }
        
        current_byte = pack_data[*offset];
        *offset += 1;
        
        size |= ((current_byte & 0x7F) as usize) << shift;
        shift += 7;
    }
    
    Ok((obj_type, size))
}

/// Decompress pack object data using a simplified approach
fn decompress_pack_object_data(
    pack_data: &[u8], 
    offset: &mut usize, 
    expected_size: usize
) -> Result<Vec<u8>, String> {
    // Find the end of compressed data by trying to decompress
    // This is a simplified approach - a full implementation would be more sophisticated
    
    let mut best_result = None;
    let mut best_end = *offset;
    
    // Try different end positions to find valid zlib data
    for end_pos in (*offset + 10)..pack_data.len().min(*offset + expected_size * 2 + 100) {
        let compressed_data = &pack_data[*offset..end_pos];
        
        if let Ok(decompressed) = crate::utils::compression::decompress_zlib(compressed_data) {
            if decompressed.len() == expected_size {
                best_result = Some(decompressed);
                best_end = end_pos;
                break;
            }
        }
    }
    
    match best_result {
        Some(decompressed) => {
            *offset = best_end;
            Ok(decompressed)
        }
        None => {
            // Fallback: try to decompress remaining data
            let compressed_data = &pack_data[*offset..pack_data.len() - 20]; // Exclude checksum
            match crate::utils::compression::decompress_zlib(compressed_data) {
                Ok(decompressed) if decompressed.len() == expected_size => {
                    *offset = pack_data.len() - 20;
                    Ok(decompressed)
                }
                _ => Err(format!("Failed to decompress object data, expected size: {}", expected_size))
            }
        }
    }
}

/// Parse tree object from pack data
fn parse_tree_from_pack(data: &[u8]) -> Result<GitObject, String> {
    let mut entries = Vec::new();
    let mut pos = 0;
    
    while pos < data.len() {
        // Parse mode (until space)
        let space_pos = data[pos..]
            .iter()
            .position(|&b| b == b' ')
            .ok_or("No space after mode")?;
        let mode = String::from_utf8_lossy(&data[pos..pos + space_pos]).to_string();
        pos += space_pos + 1;
        
        // Parse filename (until null)
        let null_pos = data[pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("No null after filename")?;
        let name = String::from_utf8_lossy(&data[pos..pos + null_pos]).to_string();
        pos += null_pos + 1;
        
        // Parse hash (20 bytes)
        if pos + 20 > data.len() {
            return Err("Truncated hash in tree".to_string());
        }
        let hash = hex::encode(&data[pos..pos + 20]);
        pos += 20;
        
        entries.push(crate::git::objects::TreeEntry::new(mode, name, hash));
    }
    
    Ok(GitObject::Tree { entries })
}

/// Parse commit object from pack data
fn parse_commit_from_pack(data: &[u8]) -> Result<GitObject, String> {
    let content = String::from_utf8_lossy(data);
    let lines: Vec<&str> = content.lines().collect();
    
    let mut tree = String::new();
    let mut parents = Vec::new();
    let mut author = String::new();
    let mut committer = String::new();
    let mut message_start = 0;
    
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("tree ") {
            tree = line[5..].to_string();
        } else if line.starts_with("parent ") {
            parents.push(line[7..].to_string());
        } else if line.starts_with("author ") {
            author = line[7..].to_string();
        } else if line.starts_with("committer ") {
            committer = line[10..].to_string();
        } else if line.is_empty() {
            message_start = i + 1;
            break;
        }
    }
    
    let message = lines[message_start..].join("\n");
    
    Ok(GitObject::Commit {
        tree,
        parents,
        author,
        committer,
        message,
    })
}

/// Parse tag object from pack data
fn parse_tag_from_pack(data: &[u8]) -> Result<GitObject, String> {
    let content = String::from_utf8_lossy(data);
    let lines: Vec<&str> = content.lines().collect();
    
    let mut object = String::new();
    let mut tag_type = String::new();
    let mut tagger = String::new();
    let mut message_start = 0;
    
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("object ") {
            object = line[7..].to_string();
        } else if line.starts_with("type ") {
            tag_type = line[5..].to_string();
        } else if line.starts_with("tagger ") {
            tagger = line[7..].to_string();
        } else if line.is_empty() {
            message_start = i + 1;
            break;
        }
    }
    
    let message = lines[message_start..].join("\n");
    
    Ok(GitObject::Tag {
        object,
        tag_type,
        tagger,
        message,
    })
}

/// Calculate hash for a Git object
fn calculate_object_hash(object: &GitObject) -> String {
    crate::utils::hash::calculate_git_hash(object)
}

/// Get human-readable object type name
fn get_object_type_name(object: &GitObject) -> &'static str {
    match object {
        GitObject::Blob { .. } => "blob",
        GitObject::Tree { .. } => "tree",
        GitObject::Commit { .. } => "commit",
        GitObject::Tag { .. } => "tag",
    }
}
