// Git Protocol Handler - Supporting both v1 and v2
// FIXED: Handle Protocol v1 fallback for push operations

use crate::bindings::theater::simple::http_types::HttpResponse;
use crate::bindings::theater::simple::runtime::log;

pub const CAPABILITIES: &str = "report-status delete-refs ofs-delta agent=git-server/0.1.0";
pub const MAX_PKT_PAYLOAD: usize = 0xFFF0 - 4; // pkt-line payload limit = 65 516
pub const MAX_SIDEBAND_DATA: usize = MAX_PKT_PAYLOAD - 1; // minus 1-byte channel

pub fn create_response(status: u16, content_type: &str, body: &[u8]) -> HttpResponse {
    let headers = vec![
        ("Content-Type".to_string(), content_type.to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
        ("Cache-Control".to_string(), "no-cache".to_string()),
    ];

    HttpResponse {
        status,
        headers,
        body: Some(body.to_vec()),
    }
}

pub fn create_error_response(message: &str) -> HttpResponse {
    let mut data = Vec::new();
    let error_line = format!("ERR {}\n", message);
    data.extend(encode_pkt_line(error_line.as_bytes()));
    data.extend(encode_flush_pkt());
    create_response(400, "application/x-git-upload-pack-result", &data)
}

pub fn create_status_response(success: bool, ref_statuses: Vec<String>) -> HttpResponse {
    create_status_response_with_capabilities(success, ref_statuses, &[])
}

pub fn create_status_response_with_capabilities(
    success: bool,
    ref_statuses: Vec<String>,
    capabilities: &[String],
) -> HttpResponse {
    let mut data = Vec::new();
    let use_sideband = capabilities.contains(&"side-band-64k".to_string());

    log(&format!(
        "Creating status response with sideband: {}",
        use_sideband
    ));

    // Unpack status
    if success {
        if use_sideband {
            data.extend(encode_status_message(b"unpack ok\n"));
        } else {
            data.extend(encode_pkt_line(b"unpack ok\n"));
        }
    } else if use_sideband {
        data.extend(encode_status_message(b"unpack failed\n"));
    } else {
        data.extend(encode_pkt_line(b"unpack failed\n"));
    }

    // Reference statuses
    for status in ref_statuses {
        let line = format!("{}\n", status);
        if use_sideband {
            data.extend(encode_status_message(line.as_bytes()));
        } else {
            data.extend(encode_pkt_line(line.as_bytes()));
        }
    }

    data.extend(encode_flush_pkt());
    create_response(200, "application/x-git-receive-pack-result", &data)
}

// ============================================================================
// Packet utilities
pub fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let total_len = data.len() + 4;
    let mut result = format!("{:04x}", total_len).into_bytes();
    result.extend_from_slice(data);
    result
}

pub fn encode_flush_pkt() -> Vec<u8> {
    b"0000".to_vec()
}

// Sideband encoding functions
pub fn encode_sideband_data(band: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 1 + payload.len());
    let len_total = 4 /*header*/ + 1 /*band*/ + payload.len();
    out.extend(format!("{len_total:04x}").as_bytes()); // <-- include the 4 bytes!
    out.push(band);
    out.extend(payload);
    out
}

pub fn encode_status_message(message: &[u8]) -> Vec<u8> {
    encode_sideband_data(1, message) // Band 1 = status messages
}
