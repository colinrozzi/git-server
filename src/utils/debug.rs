// Add this to utils/debug.rs or similar

use crate::utils::logging::safe_log as log;

pub fn hex_dump(data: &[u8], label: &str) {
    log(&format!("=== {} ({} bytes) ===", label, data.len()));
    
    let mut hex_line = String::new();
    let mut ascii_line = String::new();
    
    for (i, &byte) in data.iter().enumerate() {
        if i % 16 == 0 && i > 0 {
            log(&format!("{:08x}: {} {}", i - 16, hex_line, ascii_line));
            hex_line.clear();
            ascii_line.clear();
        }
        
        hex_line.push_str(&format!("{:02x} ", byte));
        
        if byte >= 32 && byte <= 126 {
            ascii_line.push(byte as char);
        } else {
            ascii_line.push('.');
        }
    }
    
    // Print last line
    if !hex_line.is_empty() {
        let padding = " ".repeat((16 - (data.len() % 16)) * 3);
        log(&format!("{:08x}: {}{} {}", data.len() - (data.len() % 16), hex_line, padding, ascii_line));
    }
    
    log(&format!("=== End {} ===", label));
}

pub fn analyze_pack_file(pack_data: &[u8]) {
    log("=== PACK FILE ANALYSIS ===");
    
    if pack_data.len() < 12 {
        log(&format!("ERROR: Pack too short: {} bytes", pack_data.len()));
        return;
    }
    
    // Check header
    let header = &pack_data[0..4];
    log(&format!("Header: {:?} (should be b\"PACK\")", 
                std::str::from_utf8(header).unwrap_or("invalid")));
    
    if header != b"PACK" {
        log("ERROR: Invalid pack header!");
        hex_dump(&pack_data[0..std::cmp::min(32, pack_data.len())], "Pack start");
        return;
    }
    
    // Check version
    let version = u32::from_be_bytes([pack_data[4], pack_data[5], pack_data[6], pack_data[7]]);
    log(&format!("Version: {} (should be 2)", version));
    
    // Check object count
    let obj_count = u32::from_be_bytes([pack_data[8], pack_data[9], pack_data[10], pack_data[11]]);
    log(&format!("Object count: {}", obj_count));
    
    // Show first few bytes of objects section
    if pack_data.len() > 12 {
        hex_dump(&pack_data[12..std::cmp::min(64, pack_data.len())], "First objects");
    }
    
    // Check checksum
    if pack_data.len() >= 32 {
        let checksum_start = pack_data.len() - 20;
        hex_dump(&pack_data[checksum_start..], "SHA1 checksum");
    }
}
