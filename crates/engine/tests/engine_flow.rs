use std::{path::Path, sync::Arc};

use anyhow::Result;
use memo_engine::{
    ConsolidationTrigger, EngineConfig, EntityInput, EpisodeInput, ExtractionSource, FactInput,
    MemoryEngine, MemoryLayer, MemoryRecord, RetrieveReason, RetrieveRequest,
};
use memo_model_api::EmbeddingProvider;
use tempfile::TempDir;

#[derive(Clone)]
struct TestEmbeddingProvider;

impl EmbeddingProvider for TestEmbeddingProvider {
    fn dimension(&self) -> usize {
        4
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let normalized = text.to_ascii_lowercase();
        let vector =
            if normalized.contains("happy") || text.contains("开心") || text.contains("高兴") {
                vec![1.0, 0.0, 0.0, 0.0]
            } else if normalized.contains("alice") {
                vec![0.0, 1.0, 0.0, 0.0]
            } else if normalized.contains("paris") {
                vec![0.0, 0.0, 1.0, 0.0]
            } else {
                vec![0.0, 0.0, 0.0, 1.0]
            };
        Ok(vector)
    }
}

fn open_engine(path: &Path) -> Result<MemoryEngine> {
    MemoryEngine::open(EngineConfig::new(path))
}

fn open_engine_with_vectors(path: &Path) -> Result<MemoryEngine> {
    MemoryEngine::open(
        EngineConfig::new(path).with_embedding_provider(Arc::new(TestEmbeddingProvider)),
    )
}

#[test]
fn alias_query_hits_entity_record() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.ingest_episode(EpisodeInput {
        content: "I met Alice this morning.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        confidence: 0.9,
    })?;
    drop(engine);

    let reopened = open_engine(temp.path())?;
    let result = reopened.query(RetrieveRequest {
        query: "ally".to_string(),
        limit: 5,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find(|item| matches!(&item.memory, MemoryRecord::Entity(entity) if entity.canonical_name == "Alice"))
        .expect("expected alias hit for Alice");
    assert!(entity
        .reasons
        .iter()
        .any(|reason| matches!(reason, RetrieveReason::Alias)));
    Ok(())
}

#[test]
fn bm25_query_hits_episode() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.ingest_episode(EpisodeInput {
        content: "Riverbank Robotics builds warehouse drones.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        confidence: 0.9,
    })?;

    let result = engine.query(RetrieveRequest {
        query: "warehouse drones".to_string(),
        limit: 3,
        deep: false,
    })?;

    let episode = result.results.first().expect("expected one search result");
    match &episode.memory {
        MemoryRecord::Episode(record) => assert_eq!(record.id, episode_id),
        other => panic!("expected episode result, got {other:?}"),
    }
    assert!(episode
        .reasons
        .iter()
        .any(|reason| matches!(reason, RetrieveReason::Bm25)));
    Ok(())
}

#[test]
fn vector_query_hits_semantic_neighbor() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_vectors(temp.path())?;
    let episode_id = engine.ingest_episode(EpisodeInput {
        content: "我今天很高兴".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        confidence: 0.9,
    })?;

    let result = engine.query(RetrieveRequest {
        query: "开心".to_string(),
        limit: 3,
        deep: false,
    })?;

    let episode = result
        .results
        .iter()
        .find(
            |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id),
        )
        .expect("expected vector hit");
    assert!(episode
        .reasons
        .iter()
        .any(|reason| matches!(reason, RetrieveReason::Vector)));
    Ok(())
}

#[test]
fn graph_expansion_returns_related_fact() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.ingest_episode(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![
            EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
            EntityInput {
                entity_type: "place".to_string(),
                name: "Paris".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
        ],
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "Paris".to_string(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        confidence: 0.9,
    })?;

    let result = engine.query(RetrieveRequest {
        query: "Alice".to_string(),
        limit: 5,
        deep: false,
    })?;

    let fact = result
        .results
        .iter()
        .find(
            |item| matches!(&item.memory, MemoryRecord::Fact(fact) if fact.predicate == "lives_in"),
        )
        .expect("expected graph-related fact");
    assert!(fact
        .reasons
        .iter()
        .any(|reason| matches!(reason, RetrieveReason::GraphHop { .. })));
    Ok(())
}

#[test]
fn consolidation_archives_duplicates_and_promotes_hot_memory() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let first = engine.ingest_episode(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        confidence: 0.9,
    })?;
    let second = engine.ingest_episode(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        confidence: 0.9,
    })?;

    let first_report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert_eq!(first_report.archived_records, 1);
    assert!(first_report.promoted_to_l2 >= 1);

    let first_record = engine.inspect_memory(&first)?;
    let second_record = engine.inspect_memory(&second)?;
    let (survivor_id, archived_id) = match (&first_record, &second_record) {
        (MemoryRecord::Episode(first_record), MemoryRecord::Episode(second_record))
            if first_record.archived_at.is_none() && second_record.archived_at.is_some() =>
        {
            (first_record.id.clone(), second_record.id.clone())
        }
        (MemoryRecord::Episode(first_record), MemoryRecord::Episode(second_record))
            if first_record.archived_at.is_some() && second_record.archived_at.is_none() =>
        {
            (second_record.id.clone(), first_record.id.clone())
        }
        other => panic!("unexpected duplicate consolidation state: {other:?}"),
    };

    match engine.inspect_memory(&archived_id)? {
        MemoryRecord::Episode(record) => assert!(record.archived_at.is_some()),
        other => panic!("expected archived episode, got {other:?}"),
    }

    for _ in 0..2 {
        let _ = engine.query(RetrieveRequest {
            query: "Alice likes jasmine tea.".to_string(),
            limit: 1,
            deep: false,
        })?;
    }

    let second_report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(second_report.promoted_to_l3 >= 1);

    match engine.inspect_memory(&survivor_id)? {
        MemoryRecord::Episode(record) => assert_eq!(record.layer, MemoryLayer::L3),
        other => panic!("expected surviving episode, got {other:?}"),
    }

    Ok(())
}
