#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz SkillManifest parsing from TOML
    if let Ok(s) = std::str::from_utf8(data) {
        // Try parsing as SkillManifest from TOML string
        let _: Result<cis_core::skill::manifest::SkillManifest, _> = toml::from_str(s);
    }

    // Fuzz manifest validation with partial struct
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(manifest) = toml::from_str::<cis_core::skill::manifest::SkillManifest>(s) {
            // Validate the parsed manifest
            let _ = cis_core::skill::manifest::ManifestValidator::validate(&manifest);

            // Check if it's a DAG skill and try to get DAG definition
            if manifest.is_dag_skill() {
                if let Some(dag) = manifest.dag() {
                    // Try to convert to TaskDag (this validates the DAG structure)
                    let _ = dag.to_dag();
                }
            }
        }
    }

    // Fuzz raw string as potential manifest content
    if let Ok(s) = std::str::from_utf8(data) {
        // Test skill info parsing separately
        let _: Result<cis_core::skill::manifest::SkillInfo, _> = toml::from_str(s);

        // Test task level definition parsing
        let _: Result<cis_core::skill::manifest::TaskLevelDefinition, _> = toml::from_str(s);

        // Test DAG task definition parsing
        let _: Result<cis_core::skill::manifest::DagTaskDefinition, _> = toml::from_str(s);
    }
});
