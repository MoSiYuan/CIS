#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Test DID parsing
        let _ = cis_core::identity::DIDManager::parse_did(s);
        
        // Also test DID validation
        let _ = cis_core::identity::DIDManager::is_valid_did(s);
    }
});
