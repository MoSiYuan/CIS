//! Skill manifest parsing and validation

pub mod permissions;

// Re-export from old manifest file (renamed to manifest_v1)
pub use crate::skill::manifest_v1::{
    CommandArg, ConfigField, DagDefinition, DagFileDefinition, DagPolicy,
    DagTaskDefinition, LoadBalanceStrategy, ManifestValidator, RemoteConfig,
    SkillCommand, SkillConfigSchema, SkillDependency, SkillExports, SkillInfo,
    SkillManifest, SkillPermissions, SkillType, TaskLevelDefinition,
};

pub use permissions::{
    ConstraintDeclaration, PermissionDeclaration,
};
