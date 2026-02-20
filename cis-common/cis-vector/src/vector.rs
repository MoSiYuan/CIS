use std::collections::HashMap;
use std::error::Error;

pub trait VectorIndex: Send + Sync {
    fn add(&mut self, id: &str, vector: &[f32]) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn search(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>, Box<dyn Error + Send + Sync>>;
    fn delete(&mut self, id: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn dimension(&self) -> usize;
}

pub struct VectorStore {
    dimension: usize,
    index: Box<dyn VectorIndex>,
}

impl VectorStore {
    pub fn new(dimension: usize, index: Box<dyn VectorIndex>) -> Self {
        Self { dimension, index }
    }

    pub fn add(
        &mut self,
        id: String,
        vector: Vec<f32>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if vector.len() != self.dimension {
            return Err("Invalid dimension".into());
        }
        self.index.add(&id, &vector)
    }

    pub fn search(
        &self,
        query: Vec<f32>,
        k: usize,
    ) -> Result<Vec<(String, f32)>, Box<dyn Error + Send + Sync>> {
        if query.len() != self.dimension {
            return Err("Invalid query dimension".into());
        }
        self.index.search(&query, k)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.index.delete(id)
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }
}
