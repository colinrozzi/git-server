use crate::bindings::theater::simple::runtime::log;

#[derive(Debug)]
pub struct PushRequest {
    pub ref_updates: Vec<(String, String, String)>, // (ref_name, old_oid, new_oid)
    pub pack_data: Vec<u8>,
    pub capabilities: Vec<String>, // Client-requested capabilities
}

pub fn parse_receive_pack_request(data: &[u8]) -> Result<PushRequest, String> {
    log("Parsing Protocol v1 receive-pack request");

    let mut cursor = 0;
    let mut ref_updates = Vec::new();
    let mut capabilities = Vec::new();
    let mut pack_start = 0;
    let mut first_ref = true;

    // Phase 1: Parse ref update commands
    while cursor < data.len() {
        if cursor + 4 > data.len() {
            break;
        }

        // Check for PACK signature
        if data[cursor..].starts_with(b"PACK") {
            pack_start = cursor;
            break;
        }

        let len_str =
            std::str::from_utf8(&data[cursor..cursor + 4]).map_err(|_| "Invalid packet")?;
        let len = u16::from_str_radix(len_str, 16).map_err(|_| "Invalid packet length")?;

        if len == 0 {
            cursor += 4;
            continue; // Flush packet
        }

        if len < 4 || cursor + len as usize > data.len() {
            return Err("Invalid packet".to_string());
        }

        let content = &data[cursor + 4..cursor + len as usize];
        let line = std::str::from_utf8(content)
            .map_err(|_| "Invalid UTF-8")?
            .trim_end_matches('\n');

        // Parse ref update line: "old-oid new-oid ref-name [\0capabilities]"
        if first_ref {
            // First ref may contain capabilities after null byte
            if let Some(null_pos) = line.find('\0') {
                let ref_part = &line[..null_pos];
                let cap_part = &line[null_pos + 1..];

                // Parse capabilities
                capabilities = cap_part.split_whitespace().map(|s| s.to_string()).collect();

                log(&format!("Parsed capabilities: {:?}", capabilities));

                // Parse ref update from the part before null
                let parts: Vec<&str> = ref_part.split_whitespace().collect();
                if parts.len() >= 3 {
                    ref_updates.push((
                        parts[2].to_string(), // ref name
                        parts[0].to_string(), // old oid
                        parts[1].to_string(), // new oid
                    ));
                    log(&format!(
                        "Parsed ref update: {} {} -> {}",
                        parts[2], parts[0], parts[1]
                    ));
                }
            } else {
                // No capabilities, parse as normal
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    ref_updates.push((
                        parts[2].to_string(), // ref name
                        parts[0].to_string(), // old oid
                        parts[1].to_string(), // new oid
                    ));
                    log(&format!(
                        "Parsed ref update: {} {} -> {}",
                        parts[2], parts[0], parts[1]
                    ));
                }
            }
            first_ref = false;
        } else {
            // Subsequent refs don't have capabilities
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                ref_updates.push((
                    parts[2].to_string(), // ref name
                    parts[0].to_string(), // old oid
                    parts[1].to_string(), // new oid
                ));
                log(&format!(
                    "Parsed ref update: {} {} -> {}",
                    parts[2], parts[0], parts[1]
                ));
            }
        }

        cursor += len as usize;
    }

    // Phase 2: Extract pack data
    let pack_data = if pack_start > 0 {
        data[pack_start..].to_vec()
    } else {
        Vec::new()
    };

    log(&format!(
        "Parsed {} ref updates, {} bytes pack data",
        ref_updates.len(),
        pack_data.len()
    ));

    Ok(PushRequest {
        ref_updates,
        pack_data,
        capabilities,
    })
}
