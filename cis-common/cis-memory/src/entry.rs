use chrono::{DateTime, Utc};
use cis_types::{MemoryCategory, MemoryDomain};

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MemoryEntry {
    pub fn new(
        key: String,
        value: Vec<u8>,
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Self {
        let now = Utc::now();
        Self {
            key,
            value,
            domain,
            category,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update_value(&mut self, value: Vec<u8>) {
        self.value = value;
        self.updated_at = Utc::now();
    }
}
