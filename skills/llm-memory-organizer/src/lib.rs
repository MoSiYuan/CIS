//! LLM Memory Organizer Skill
//!
//! 使用 AI (Claude CLI) 整理和增强记忆

use cis_skill_sdk::{memory, log};

pub struct MemoryOrganizer;

impl MemoryOrganizer {
    pub async fn organize(key: &str, content: &str) {
        log::info("Organizing memory: {}", key);
        
        // 提取关键词
        let prompt = format!(
            "Extract 3-5 keywords from:\n{}",
            content
        );
        
        match cis_skill_sdk::ai::chat(&prompt).await {
            Ok(keywords) => {
                let _ = memory::set(
                    &format!("_meta/{}/keywords", key),
                    keywords.as_bytes()
                );
            }
            Err(e) => log::error("Failed: {}", e),
        }
    }
}

#[no_mangle]
pub extern "C" fn skill_init() -> i32 {
    log::info("Memory Organizer initialized");
    0
}
