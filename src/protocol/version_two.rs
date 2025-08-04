use crate::bindings::theater::simple::runtime::log;
use crate::protocol::http::encode_pkt_line;

#[derive(Debug)]
pub struct CommandRequest {
    pub command: String,
    pub capabilities: Vec<String>,
    pub args: Vec<String>,
}

pub fn parse_command_request(data: &[u8]) -> Result<CommandRequest, String> {
    log(&format!(
        "Parsing Protocol v2 command request, data length: {} bytes",
        data.len()
    ));

    let mut lines = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if pos + 4 > data.len() {
            break;
        }

        let len_bytes = &data[pos..pos + 4];
        let len_str = std::str::from_utf8(len_bytes).map_err(|_| "Invalid packet")?;
        let len = u16::from_str_radix(len_str, 16).map_err(|_| "Invalid packet length")?;

        if len == 0 {
            // Flush packet - end of request
            pos += 4;
            break;
        }

        if len == 1 {
            // Delimiter packet - continue
            pos += 4;
            continue;
        }

        if len < 4 {
            return Err(format!("Invalid packet length: {} (must be >= 4)", len));
        }

        if pos + len as usize > data.len() {
            return Err(format!(
                "Packet extends beyond data: need {} bytes, have {}",
                len,
                data.len() - pos
            ));
        }

        let content = &data[pos + 4..pos + len as usize];
        let line = std::str::from_utf8(content)
            .map_err(|e| format!("Invalid UTF-8 in packet content: {}", e))?
            .trim_end_matches('\n');

        if !line.is_empty() {
            lines.push(line.to_string());
        }
        pos += len as usize;
    }

    if lines.is_empty() {
        return Err("No command found in request".to_string());
    }

    let first_line = &lines[0];
    let command = if let Some(cmd) = first_line.strip_prefix("command=") {
        cmd.to_string()
    } else {
        return Err(format!("Invalid command format: {}", first_line));
    };

    log(&format!(
        "Parsed Protocol v2 command: '{}' with {} args",
        command,
        lines.len() - 1
    ));

    Ok(CommandRequest {
        command,
        capabilities: vec![],
        args: lines[1..].to_vec(),
    })
}
