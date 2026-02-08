#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Test WebSocket network message parsing from JSON bytes
    let _: Result<cis_core::network::WsNetworkMessage, _> = serde_json::from_slice(data);
});
