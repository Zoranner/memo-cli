use std::sync::Arc;

use memo_engine::{
    eval::{run_recall_eval, EvalDataset},
    DreamTrigger, EmbeddingProvider, EngineConfig, EntityInput, EpisodeInput, ExtractionSource,
    FactInput, MemoryEngine, MemoryLayer, RestoreScope,
};
use tempfile::TempDir;

const SYNTHETIC_BASIC: &str = include_str!("../../../../evals/synthetic/basic.json");

pub fn open_engine() -> (TempDir, MemoryEngine) {
    let temp = TempDir::new().expect("temp dir");
    let engine = MemoryEngine::open(EngineConfig::new(temp.path())).expect("open engine");
    (temp, engine)
}

pub fn episode(content: impl Into<String>) -> EpisodeInput {
    EpisodeInput {
        content: content.into(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.85,
    }
}

pub fn seed_alias_engine() -> (TempDir, MemoryEngine) {
    let (temp, engine) = open_engine();
    engine.remember(alice_episode()).expect("remember alice");
    (temp, engine)
}

pub fn seed_bm25_engine() -> (TempDir, MemoryEngine) {
    let (temp, engine) = open_engine();
    engine
        .remember(episode("Riverbank Robotics builds warehouse drones."))
        .expect("remember riverbank");
    engine
        .restore_full(RestoreScope::Text)
        .expect("restore text index");
    (temp, engine)
}

pub fn seed_graph_engine() -> (TempDir, MemoryEngine) {
    let (temp, engine) = open_engine();
    engine
        .remember(alice_fact_episode())
        .expect("remember alice fact");
    (temp, engine)
}

pub fn seed_current_state_engine() -> (TempDir, MemoryEngine) {
    let (temp, engine) = open_engine();
    let mut dataset: EvalDataset =
        serde_json::from_str(SYNTHETIC_BASIC).expect("synthetic eval dataset should parse");
    dataset.cases.clear();
    run_recall_eval(&engine, dataset).expect("seed synthetic eval memories");
    engine.dream(DreamTrigger::Manual).expect("run dream");
    engine
        .restore_full(RestoreScope::Text)
        .expect("restore text index");
    (temp, engine)
}

pub fn seed_vector_engine() -> (TempDir, MemoryEngine) {
    let temp = TempDir::new().expect("temp dir");
    let engine = MemoryEngine::open(
        EngineConfig::new(temp.path())
            .with_embedding_provider(Arc::new(DeterministicEmbeddingProvider)),
    )
    .expect("open engine");
    engine
        .remember(episode("Riverbank Robotics builds warehouse drones."))
        .expect("remember vector target");
    engine
        .restore_full(RestoreScope::Vector)
        .expect("restore vector index");
    (temp, engine)
}

fn alice_episode() -> EpisodeInput {
    EpisodeInput {
        content: "Alice is also known as Ally.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.9,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.85,
    }
}

fn alice_fact_episode() -> EpisodeInput {
    EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![
            EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: Vec::new(),
                confidence: 0.9,
                source: ExtractionSource::Manual,
            },
            EntityInput {
                entity_type: "place".to_string(),
                name: "Paris".to_string(),
                aliases: Vec::new(),
                confidence: 0.9,
                source: ExtractionSource::Manual,
            },
        ],
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "Paris".to_string(),
            confidence: 0.9,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.85,
    }
}

struct DeterministicEmbeddingProvider;

impl EmbeddingProvider for DeterministicEmbeddingProvider {
    fn dimension(&self) -> usize {
        8
    }

    fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let n = text.to_ascii_lowercase();
        let mut v = vec![0.0; 8];
        if n.contains("warehouse") || n.contains("drone") {
            v[0] = 1.0;
        }
        if n.contains("alice") || n.contains("ally") || n.contains("morgan") {
            v[1] = 1.0;
        }
        if n.contains("london") || n.contains("paris") || n.contains("berlin") {
            v[2] = 1.0;
        }
        if n.contains("riverbank") || n.contains("robotics") {
            v[3] = 1.0;
        }
        if n.contains("aurora") || n.contains("project") {
            v[4] = 1.0;
        }
        if n.contains("evaluator") || n.contains("offline") {
            v[5] = 1.0;
        }
        if n.contains("bob") || n.contains("bobby") {
            v[6] = 1.0;
        }
        v[7] = (n.len() % 13) as f32 / 13.0;
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        Ok(v)
    }
}
