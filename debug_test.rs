use super::*;

#[test] 
fn debug_parser() {
    let test_data = b"0063340e325d1b85b3c0d5d7d8c5d46efad08fcd8 0000000000000000000000000000000000000000 refs/heads/main\n0000PACK...";
    
    println!("Test data: {:?}", std::str::from_utf8(test_data).unwrap_or("invalid utf8"));
    println!("Test data length: {}", test_data.len());
    
    let result = ProtocolV2Parser::parse_receive_pack_request(test_data);
    
    match result {
        Ok(request) => {
            println!("Success! Ref updates: {}", request.ref_updates.len());
            for (i, update) in request.ref_updates.iter().enumerate() {
                println!("  Update {}: {} {} -> {}", i, update.ref_name, update.old_oid, update.new_oid);
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
