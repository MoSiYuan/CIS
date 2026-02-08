#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz DAG definition parsing from TOML
    if let Ok(s) = std::str::from_utf8(data) {
        // Try parsing as DagDefinition TOML
        let _: Result<cis_core::skill::manifest::DagDefinition, _> = toml::from_str(s);
    }

    // Fuzz JSON DAG parsing
    if let Ok(s) = std::str::from_utf8(data) {
        // Try parsing as JSON DagDefinition
        let _: Result<cis_core::skill::manifest::DagDefinition, _> = serde_json::from_str(s);
    }

    // Fuzz DAG event parsing
    if let Ok(s) = std::str::from_utf8(data) {
        // Try parsing as DAG execute event
        let _ = cis_core::matrix::events::dag::parse_dag_event(s);
    }
});
