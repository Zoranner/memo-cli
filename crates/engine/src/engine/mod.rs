use std::{collections::HashMap, sync::Mutex};

use anyhow::{Context, Result};

use crate::{
    db::Database,
    text_index::TextIndex,
    types::{EngineConfig, MemoryRecord, RecallReason},
    vector_index::VectorIndex,
};

mod dream;
mod ingest;
mod recall;
mod restore;

pub type Engine = MemoryEngine;

pub struct MemoryEngine {
    config: EngineConfig,
    db: Database,
    text_index: Mutex<TextIndex>,
    vector_index: Mutex<VectorIndex>,
    l3_cache: Mutex<HashMap<String, MemoryRecord>>,
    session: Mutex<SessionCache>,
}

#[derive(Default)]
struct SessionCache {
    recent_aliases: HashMap<String, String>,
    recent_memory_ids: Vec<String>,
    recent_topics: Vec<String>,
}

#[derive(Clone)]
struct Candidate {
    memory: MemoryRecord,
    score: f32,
    reasons: Vec<RecallReason>,
}

impl MemoryEngine {
    pub fn open(config: EngineConfig) -> Result<Self> {
        config.ensure_dirs()?;

        let db = Database::open(&config.sqlite_path())?;
        let text_index = TextIndex::open(&config.text_index_dir())?;
        let vector_index = VectorIndex::open(config.vector_index_path(), config.vector_dimension)?;

        let engine = Self {
            config,
            db,
            text_index: Mutex::new(text_index),
            vector_index: Mutex::new(vector_index),
            l3_cache: Mutex::new(HashMap::new()),
            session: Mutex::new(SessionCache::default()),
        };

        engine.refresh_l3_cache()?;
        Ok(engine)
    }

    pub fn reflect(&self, id: &str) -> Result<MemoryRecord> {
        self.db
            .get_memory(id)?
            .with_context(|| format!("memory not found: {}", id))
    }
}
