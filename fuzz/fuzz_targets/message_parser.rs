#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz WebSocket message parsing
    if let Ok(s) = std::str::from_utf8(data) {
        // Try parsing as WebSocket message
        let _: Result<cis_core::matrix::websocket::protocol::WsMessage, _> =
            serde_json::from_str(s);
    }

    // Fuzz Matrix event parsing
    if let Ok(s) = std::str::from_utf8(data) {
        // Try parsing as event message
        let _: Result<cis_core::matrix::websocket::protocol::EventMessage, _> =
            serde_json::from_str(s);

        // Try parsing as handshake message
        let _: Result<cis_core::matrix::websocket::protocol::HandshakeMessage, _> =
            serde_json::from_str(s);

        // Try parsing as auth message
        let _: Result<cis_core::matrix::websocket::protocol::AuthMessage, _> =
            serde_json::from_str(s);

        // Try parsing as ping/pong messages
        let _: Result<cis_core::matrix::websocket::protocol::PingMessage, _> =
            serde_json::from_str(s);
        let _: Result<cis_core::matrix::websocket::protocol::PongMessage, _> =
            serde_json::from_str(s);

        // Try parsing as error message
        let _: Result<cis_core::matrix::websocket::protocol::ErrorMessage, _> =
            serde_json::from_str(s);

        // Try parsing as acknowledgment message
        let _: Result<cis_core::matrix::websocket::protocol::AckMessage, _> =
            serde_json::from_str(s);

        // Try parsing as sync request/response
        let _: Result<cis_core::matrix::websocket::protocol::SyncRequest, _> =
            serde_json::from_str(s);
        let _: Result<cis_core::matrix::websocket::protocol::SyncResponse, _> =
            serde_json::from_str(s);
    }

    // Fuzz sync filter parsing
    if let Ok(s) = std::str::from_utf8(data) {
        let _: Result<cis_core::matrix::websocket::protocol::SyncFilter, _> =
            serde_json::from_str(s);
    }

    // Fuzz skill event parsing
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(s) {
            // Try parsing various event types
            let event_types = [
                "skill.register",
                "skill.execute",
                "skill.response",
                "skill.error",
                "skill.status",
            ];

            for event_type in &event_types {
                let _ =
                    cis_core::matrix::events::skill::SkillEvent::parse_event(event_type, &value);
            }
        }
    }

    // Fuzz DAG-related event parsing
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = cis_core::matrix::events::dag::parse_todo_proposal_event(s);
        let _ = cis_core::matrix::events::dag::parse_todo_proposal_response(s);
    }
});
