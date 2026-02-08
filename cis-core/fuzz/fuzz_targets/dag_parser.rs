#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Test DAG definition parsing from JSON
        let _: Result<cis_core::skill::manifest::DagDefinition, _> = serde_json::from_str(s);
        
        // Also test TOML parsing since DAG can be defined in TOML format too
        let _: Result<cis_core::skill::manifest::SkillManifest, _> = cis_core::skill::manifest::SkillManifest::from_str(s);
    }
});
