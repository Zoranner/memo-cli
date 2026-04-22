use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use hnsw_rs::prelude::{AnnT, DistCosine, Hnsw};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct VectorHit {
    pub id: String,
    pub score: f32,
}

pub enum VectorUpdate {
    Upsert {
        kind: String,
        id: String,
        vector: Vec<f32>,
    },
    Delete {
        kind: String,
        id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVector {
    ann_id: usize,
    id: String,
    kind: String,
    vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVectorDisk {
    #[serde(default)]
    ann_id: Option<usize>,
    id: String,
    kind: String,
    vector: Vec<f32>,
}

type AnnIndex = Hnsw<'static, f32, DistCosine>;

pub struct VectorIndex {
    path: PathBuf,
    dimension: usize,
    records: HashMap<String, StoredVector>,
    ann: AnnIndex,
    record_ids_by_ann_id: HashMap<usize, String>,
    next_ann_id: usize,
    defer_commit: bool,
}

impl VectorIndex {
    pub fn open(path: PathBuf, dimension: usize) -> Result<Self> {
        let records = load_records(&path)?;
        let next_ann_id = records
            .values()
            .map(|record| record.ann_id)
            .max()
            .map_or(0, |ann_id| ann_id.saturating_add(1));
        let ann = build_hnsw(dimension, &records)?;
        let record_ids_by_ann_id = build_ann_id_map(&records);
        Ok(Self {
            path,
            dimension,
            records,
            ann,
            record_ids_by_ann_id,
            next_ann_id,
            defer_commit: false,
        })
    }

    pub fn upsert(&mut self, kind: &str, id: &str, vector: &[f32]) -> Result<()> {
        self.ensure_dimension(vector)?;
        self.insert_record(kind, id, vector);
        if self.defer_commit {
            return Ok(());
        }
        self.reindex()?;
        self.persist()
    }

    pub fn rebuild(&mut self, docs: &[(String, String, Vec<f32>)]) -> Result<usize> {
        self.records.clear();
        self.with_deferred_commit(|index| {
            for (id, kind, vector) in docs {
                index.upsert(kind, id, vector)?;
            }
            Ok(())
        })?;
        self.reindex()?;
        self.persist()?;
        Ok(self.records.len())
    }

    pub fn apply_updates(&mut self, updates: &[VectorUpdate]) -> Result<usize> {
        self.with_deferred_commit(|index| {
            for update in updates {
                match update {
                    VectorUpdate::Upsert { kind, id, vector } => index.upsert(kind, id, vector)?,
                    VectorUpdate::Delete { kind, id } => index.delete(kind, id)?,
                }
            }
            Ok(())
        })?;
        self.reindex()?;
        self.persist()?;
        Ok(self.document_count())
    }

    fn with_deferred_commit<T>(
        &mut self,
        action: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.defer_commit;
        self.defer_commit = true;
        let result = action(self);
        self.defer_commit = previous;
        result
    }

    fn insert_record(&mut self, kind: &str, id: &str, vector: &[f32]) {
        let key = storage_key(kind, id);
        let ann_id = self
            .records
            .get(&key)
            .map(|record| record.ann_id)
            .unwrap_or_else(|| self.allocate_ann_id());
        self.records.insert(
            key,
            StoredVector {
                ann_id,
                id: id.to_string(),
                kind: kind.to_string(),
                vector: vector.to_vec(),
            },
        );
    }

    pub fn delete(&mut self, kind: &str, id: &str) -> Result<()> {
        let removed = self.records.remove(&storage_key(kind, id));
        if removed.is_none() || self.defer_commit {
            return Ok(());
        }
        self.reindex()?;
        self.persist()
    }

    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<VectorHit>> {
        if query.len() != self.dimension || limit == 0 || self.records.is_empty() {
            return Ok(Vec::new());
        }

        let ef = search_ef(limit, self.records.len());
        let mut hits = self
            .ann
            .search(query, limit.min(self.records.len()), ef)
            .into_iter()
            .filter_map(|neighbor| {
                self.record_ids_by_ann_id
                    .get(&neighbor.d_id)
                    .cloned()
                    .map(|id| VectorHit {
                        id,
                        score: (1.0 - neighbor.distance).clamp(-1.0, 1.0),
                    })
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| right.score.total_cmp(&left.score));
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
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.id.cmp(&b.id)));
        std::fs::write(&self.path, serde_json::to_vec(&records)?)?;
        if self.records.is_empty() {
            cleanup_sidecars(&self.path)?;
            return Ok(());
        }

        let basename = dump_basename(&self.path)?;
        let dumped_basename = self.ann.file_dump(parent, &basename)?;
        if dumped_basename != basename {
            anyhow::bail!(
                "unexpected vector index dump basename: expected {}, got {}",
                basename,
                dumped_basename
            );
        }
        Ok(())
    }

    fn reindex(&mut self) -> Result<()> {
        self.ann = build_hnsw(self.dimension, &self.records)?;
        self.record_ids_by_ann_id = build_ann_id_map(&self.records);
        Ok(())
    }

    fn allocate_ann_id(&mut self) -> usize {
        let ann_id = self.next_ann_id;
        self.next_ann_id = self.next_ann_id.saturating_add(1);
        ann_id
    }

    pub fn document_count(&self) -> usize {
        self.records.len()
    }
}

fn load_records(path: &Path) -> Result<HashMap<String, StoredVector>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let raw = std::fs::read(path)
        .with_context(|| format!("failed to read vector index file: {}", path.display()))?;
    if raw.is_empty() {
        return Ok(HashMap::new());
    }

    let entries = serde_json::from_slice::<Vec<StoredVectorDisk>>(&raw)
        .with_context(|| format!("failed to decode vector index file: {}", path.display()))?;
    Ok(entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let ann_id = entry.ann_id.unwrap_or(index);
            let record = StoredVector {
                ann_id,
                id: entry.id,
                kind: entry.kind,
                vector: entry.vector,
            };
            (storage_key(&record.kind, &record.id), record)
        })
        .collect())
}

fn storage_key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

fn build_hnsw(dimension: usize, records: &HashMap<String, StoredVector>) -> Result<AnnIndex> {
    let mut ann = new_hnsw(records.len());
    let mut ordered = records.values().collect::<Vec<_>>();
    ordered.sort_by_key(|record| record.ann_id);

    for record in ordered {
        if record.vector.len() != dimension {
            anyhow::bail!(
                "vector dimension mismatch for {} {}: expected {}, got {}",
                record.kind,
                record.id,
                dimension,
                record.vector.len()
            );
        }
        ann.insert((&record.vector, record.ann_id));
    }
    ann.set_searching_mode(true);
    Ok(ann)
}

fn new_hnsw(record_count: usize) -> AnnIndex {
    const MAX_CONNECTIONS: usize = 24;
    const EF_CONSTRUCTION: usize = 200;
    const MAX_LAYERS: usize = 16;

    let max_elements = record_count.max(1);
    let mut ann = Hnsw::<f32, DistCosine>::new(
        MAX_CONNECTIONS,
        max_elements,
        MAX_LAYERS,
        EF_CONSTRUCTION,
        DistCosine {},
    );
    ann.set_extend_candidates(true);
    ann.set_keeping_pruned(true);
    ann
}

fn build_ann_id_map(records: &HashMap<String, StoredVector>) -> HashMap<usize, String> {
    records
        .values()
        .map(|record| (record.ann_id, record.id.clone()))
        .collect()
}

fn search_ef(limit: usize, record_count: usize) -> usize {
    let base = limit.max(8).saturating_mul(4);
    base.min(record_count.max(limit))
}

fn dump_basename(path: &Path) -> Result<String> {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .context("vector index path must include a file name")
}

fn cleanup_sidecars(path: &Path) -> Result<()> {
    for sidecar in [
        path.with_extension("hnsw.graph"),
        path.with_extension("hnsw.data"),
    ] {
        if sidecar.exists() {
            std::fs::remove_file(&sidecar).with_context(|| {
                format!(
                    "failed to remove vector index sidecar: {}",
                    sidecar.display()
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::VectorIndex;

    #[test]
    fn rebuild_writes_hnsw_sidecar_files() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("vector-index.json");
        let mut index = VectorIndex::open(path.clone(), 3).expect("open index");

        index
            .rebuild(&[
                (
                    "episode-1".to_string(),
                    "episode".to_string(),
                    vec![1.0, 0.0, 0.0],
                ),
                (
                    "episode-2".to_string(),
                    "episode".to_string(),
                    vec![0.0, 1.0, 0.0],
                ),
            ])
            .expect("rebuild index");

        assert!(path.exists(), "manifest should be persisted");
        assert!(
            temp.path().join("vector-index.hnsw.graph").exists(),
            "expected HNSW graph dump"
        );
        assert!(
            temp.path().join("vector-index.hnsw.data").exists(),
            "expected HNSW data dump"
        );
    }

    #[test]
    fn reopen_restores_searchable_vectors() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("vector-index.json");

        {
            let mut index = VectorIndex::open(path.clone(), 3).expect("open index");
            index
                .rebuild(&[
                    (
                        "episode-1".to_string(),
                        "episode".to_string(),
                        vec![1.0, 0.0, 0.0],
                    ),
                    (
                        "episode-2".to_string(),
                        "episode".to_string(),
                        vec![0.0, 1.0, 0.0],
                    ),
                ])
                .expect("rebuild index");
        }

        let reopened = VectorIndex::open(path, 3).expect("reopen index");
        let hits = reopened
            .search(&[0.95, 0.05, 0.0], 1)
            .expect("search persisted index");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "episode-1");
    }
}
