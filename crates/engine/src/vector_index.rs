use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct VectorHit {
    pub id: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVector {
    id: String,
    kind: String,
    vector: Vec<f32>,
}

pub struct VectorIndex {
    path: PathBuf,
    dimension: usize,
    records: HashMap<String, StoredVector>,
}

impl VectorIndex {
    pub fn open(path: PathBuf, dimension: usize) -> Result<Self> {
        let records = load_records(&path)?;
        Ok(Self {
            path,
            dimension,
            records,
        })
    }

    pub fn upsert(&mut self, kind: &str, id: &str, vector: &[f32]) -> Result<()> {
        self.ensure_dimension(vector)?;
        self.records.insert(
            storage_key(kind, id),
            StoredVector {
                id: id.to_string(),
                kind: kind.to_string(),
                vector: vector.to_vec(),
            },
        );
        self.persist()
    }

    pub fn rebuild(&mut self, docs: &[(String, String, Vec<f32>)]) -> Result<usize> {
        self.records.clear();
        for (id, kind, vector) in docs {
            self.upsert(kind, id, vector)?;
        }
        self.persist()?;
        Ok(self.records.len())
    }

    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<VectorHit>> {
        if query.len() != self.dimension || limit == 0 || self.records.is_empty() {
            return Ok(Vec::new());
        }

        let mut hits = self
            .records
            .values()
            .filter_map(|record| {
                cosine_similarity(query, &record.vector).map(|score| VectorHit {
                    id: record.id.clone(),
                    score,
                })
            })
            .collect::<Vec<_>>();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        Ok(hits)
    }

    fn ensure_dimension(&self, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimension {
            anyhow::bail!(
                "vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            );
        }
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.id.cmp(&b.id)));
        std::fs::write(&self.path, serde_json::to_vec(&records)?)?;
        Ok(())
    }
}

fn load_records(path: &PathBuf) -> Result<HashMap<String, StoredVector>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let raw = std::fs::read(path)
        .with_context(|| format!("failed to read vector index file: {}", path.display()))?;
    if raw.is_empty() {
        return Ok(HashMap::new());
    }

    let entries = serde_json::from_slice::<Vec<StoredVector>>(&raw)
        .with_context(|| format!("failed to decode vector index file: {}", path.display()))?;
    Ok(entries
        .into_iter()
        .map(|entry| (storage_key(&entry.kind, &entry.id), entry))
        .collect())
}

fn storage_key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (lhs, rhs) in a.iter().zip(b.iter()) {
        dot += lhs * rhs;
        norm_a += lhs * lhs;
        norm_b += rhs * rhs;
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator <= f32::EPSILON {
        return None;
    }

    Some((dot / denominator).clamp(-1.0, 1.0))
}
