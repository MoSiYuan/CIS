use super::vector::VectorIndex;
use std::collections::HashMap;
use std::error::Error;

pub struct HnswIndex {
    dimension: usize,
    max_elements: usize,
    vectors: HashMap<String, Vec<f32>>,
}

impl HnswIndex {
    pub fn new(dimension: usize, max_elements: usize) -> Self {
        Self {
            dimension,
            max_elements,
            vectors: HashMap::new(),
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

impl VectorIndex for HnswIndex {
    fn add(&mut self, id: &str, vector: &[f32]) -> Result<(), Box<dyn Error + Send + Sync>> {
        if vector.len() != self.dimension {
            return Err("Invalid dimension".into());
        }
        self.vectors.insert(id.to_string(), vector.to_vec());
        Ok(())
    }

    fn search(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>, Box<dyn Error + Send + Sync>> {
        let mut scores: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(id, vec)| (id.clone(), Self::cosine_similarity(query, vec)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scores.into_iter().take(k).collect())
    }

    fn delete(&mut self, id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.vectors.remove(id);
        Ok(())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
