use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;
use memo_engine::{
    DreamTrigger, EmbeddingProvider, EngineConfig, EntityInput, EpisodeInput, ExtractedEntity,
    ExtractedFact, ExtractionProvider, ExtractionResult, ExtractionSource, FactInput, MemoryEngine,
    MemoryLayer, MemoryRecord, RecallReason, RecallRequest, RerankProvider, RerankScore,
    RestoreScope,
};
use rusqlite::Connection;
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

#[derive(Clone)]
struct CountingEmbeddingProvider {
    calls: Arc<AtomicUsize>,
}

impl EmbeddingProvider for CountingEmbeddingProvider {
    fn dimension(&self) -> usize {
        4
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        TestEmbeddingProvider.embed_text(text)
    }
}

#[derive(Clone)]
struct FailingEmbeddingProvider;

impl EmbeddingProvider for FailingEmbeddingProvider {
    fn dimension(&self) -> usize {
        4
    }

    fn embed_text(&self, _text: &str) -> Result<Vec<f32>> {
        anyhow::bail!("embedding backend unavailable")
    }
}

#[derive(Clone)]
struct TestExtractionProvider;

impl ExtractionProvider for TestExtractionProvider {
    fn extract(&self, text: &str) -> Result<ExtractionResult> {
        if text.contains("Alice lives in Paris") {
            Ok(ExtractionResult {
                entities: vec![
                    ExtractedEntity {
                        entity_type: "person".to_string(),
                        name: "Alice".to_string(),
                        aliases: vec!["Ally".to_string()],
                        confidence: 0.93,
                    },
                    ExtractedEntity {
                        entity_type: "place".to_string(),
                        name: "Paris".to_string(),
                        aliases: Vec::new(),
                        confidence: 0.92,
                    },
                ],
                facts: vec![ExtractedFact {
                    subject: "Alice".to_string(),
                    predicate: "lives_in".to_string(),
                    object: "Paris".to_string(),
                    confidence: 0.9,
                }],
            })
        } else {
            Ok(ExtractionResult::default())
        }
    }
}

#[derive(Clone)]
struct TestRerankProvider;

impl RerankProvider for TestRerankProvider {
    fn rerank(&self, _query: &str, documents: &[String]) -> Result<Vec<RerankScore>> {
        let mut scores = documents
            .iter()
            .enumerate()
            .map(|(index, document)| RerankScore {
                index,
                score: if document.contains("travel checklist") {
                    10.0
                } else {
                    0.1
                },
            })
            .collect::<Vec<_>>();
        scores.sort_by(|left, right| right.score.total_cmp(&left.score));
        Ok(scores)
    }
}

#[derive(Clone)]
struct FailingRerankProvider;

impl RerankProvider for FailingRerankProvider {
    fn rerank(&self, _query: &str, _documents: &[String]) -> Result<Vec<RerankScore>> {
        anyhow::bail!("rerank backend unavailable")
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

fn open_engine_with_extraction(path: &Path) -> Result<MemoryEngine> {
    MemoryEngine::open(
        EngineConfig::new(path).with_extraction_provider(Arc::new(TestExtractionProvider)),
    )
}

fn open_engine_with_rerank(path: &Path) -> Result<MemoryEngine> {
    MemoryEngine::open(EngineConfig::new(path).with_rerank_provider(Arc::new(TestRerankProvider)))
}

fn open_engine_with_failing_embeddings(path: &Path) -> Result<MemoryEngine> {
    MemoryEngine::open(
        EngineConfig::new(path).with_embedding_provider(Arc::new(FailingEmbeddingProvider)),
    )
}

fn open_engine_with_failing_rerank(path: &Path) -> Result<MemoryEngine> {
    MemoryEngine::open(
        EngineConfig::new(path).with_rerank_provider(Arc::new(FailingRerankProvider)),
    )
}

#[test]
fn preview_remember_merges_provider_and_manual_inputs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_extraction(temp.path())?;

    let preview = engine.preview_remember(&EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Bob".to_string(),
            aliases: Vec::new(),
            confidence: 0.8,
            source: ExtractionSource::Manual,
        }],
        facts: vec![FactInput {
            subject: "Bob".to_string(),
            predicate: "knows".to_string(),
            object: "Alice".to_string(),
            confidence: 0.8,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    assert!(preview.entities.iter().any(|entity| entity.name == "Alice"));
    assert!(preview.entities.iter().any(|entity| entity.name == "Bob"));
    assert!(preview
        .facts
        .iter()
        .any(|fact| fact.predicate == "lives_in" && fact.object == "Paris"));
    assert!(preview
        .facts
        .iter()
        .any(|fact| fact.predicate == "knows" && fact.subject == "Bob"));
    Ok(())
}

#[test]
fn alias_query_hits_entity_record() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.remember(EpisodeInput {
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
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    drop(engine);

    let reopened = open_engine(temp.path())?;
    let result = reopened.recall(RecallRequest {
        query: "ally".to_string(),
        limit: 5,
        deep: false,
    })?;

    assert!(!result.deep_search_used);
    let entity = result
        .results
        .iter()
        .find(|item| matches!(&item.memory, MemoryRecord::Entity(entity) if entity.canonical_name == "Alice"))
        .expect("expected alias hit for Alice");
    assert!(entity
        .reasons
        .iter()
        .any(|reason| matches!(reason, RecallReason::Alias)));
    Ok(())
}

#[test]
fn remember_entity_alias_reuses_existing_entity_record() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.remember(EpisodeInput {
        content: "I met Alice this morning.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    engine.remember(EpisodeInput {
        content: "Ally sent a follow-up.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Ally".to_string(),
            aliases: Vec::new(),
            confidence: 0.8,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let total_entities: i64 =
        conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
    let alice_id: String = conn.query_row(
        "SELECT id FROM entities WHERE canonical_name = 'Alice' LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let mention_entity_id: String = conn.query_row(
        "SELECT entity_id
         FROM mentions
         WHERE episode_id = (
             SELECT id FROM episodes WHERE content = 'Ally sent a follow-up.' LIMIT 1
         )
         LIMIT 1",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(total_entities, 1);
    assert_eq!(mention_entity_id, alice_id);
    Ok(())
}

#[test]
fn bm25_query_hits_episode() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "Riverbank Robotics builds warehouse drones.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let result = engine.recall(RecallRequest {
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
        .any(|reason| matches!(reason, RecallReason::Bm25)));
    Ok(())
}

#[test]
fn vector_query_hits_semantic_neighbor() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_vectors(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "我今天很高兴".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Vector)?;

    let result = engine.recall(RecallRequest {
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
        .any(|reason| matches!(reason, RecallReason::Vector)));
    Ok(())
}

#[test]
fn recall_skips_query_embedding_when_vector_index_is_empty() -> Result<()> {
    let temp = TempDir::new()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()).with_embedding_provider(
        Arc::new(CountingEmbeddingProvider {
            calls: Arc::clone(&calls),
        }),
    ))?;

    engine.remember(EpisodeInput {
        content: "我今天很高兴".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    let calls_after_remember = calls.load(Ordering::SeqCst);

    let _ = engine.recall(RecallRequest {
        query: "开心".to_string(),
        limit: 3,
        deep: false,
    })?;

    assert_eq!(calls.load(Ordering::SeqCst), calls_after_remember);
    Ok(())
}

#[test]
fn recall_falls_back_to_non_vector_paths_when_query_embedding_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_failing_embeddings(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Riverbank Robotics builds warehouse drones.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let result = engine.recall(RecallRequest {
        query: "warehouse drones".to_string(),
        limit: 3,
        deep: false,
    })?;

    assert!(result.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.content == "Riverbank Robotics builds warehouse drones.")
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Bm25))
    ));
    Ok(())
}

#[test]
fn remember_fact_only_creates_mentions_for_fallback_entities() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
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
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let mention_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM mentions WHERE episode_id = ?1",
        rusqlite::params![episode_id],
        |row| row.get(0),
    )?;

    assert_eq!(mention_count, 2);
    Ok(())
}

#[test]
fn remember_fact_alias_reuses_existing_entity_record() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Alice is also called Ally.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let episode_id = engine.remember(EpisodeInput {
        content: "Ally lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: vec![FactInput {
            subject: "Ally".to_string(),
            predicate: "lives_in".to_string(),
            object: "Paris".to_string(),
            confidence: 0.9,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let alice_id: String = conn.query_row(
        "SELECT id FROM entities WHERE canonical_name = 'Alice' LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let subject_entity_id: String = conn.query_row(
        "SELECT subject_entity_id FROM facts WHERE source_episode_id = ?1 LIMIT 1",
        rusqlite::params![episode_id],
        |row| row.get(0),
    )?;
    let ally_unknown_count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM entities
         WHERE canonical_name = 'Ally' AND entity_type = 'unknown'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(subject_entity_id, alice_id);
    assert_eq!(ally_unknown_count, 0);
    Ok(())
}

#[test]
fn explicit_entities_upgrade_unknown_fact_entities_to_typed_records() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
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
        confidence: 0.9,
    })?;

    engine.remember(EpisodeInput {
        content: "Alice is a traveler in Paris.".to_string(),
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
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let alice_type: String = conn.query_row(
        "SELECT entity_type FROM entities WHERE canonical_name = 'Alice' LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let paris_type: String = conn.query_row(
        "SELECT entity_type FROM entities WHERE canonical_name = 'Paris' LIMIT 1",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(alice_type, "person");
    assert_eq!(paris_type, "place");
    Ok(())
}

#[test]
fn graph_expansion_returns_related_fact() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.remember(EpisodeInput {
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
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let result = engine.recall(RecallRequest {
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
        .any(|reason| matches!(reason, RecallReason::GraphHop { .. })));
    Ok(())
}

#[test]
fn deep_query_uses_rerank_to_promote_best_candidate() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_rerank(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Paris travel checklist for May.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "place".to_string(),
            name: "Paris".to_string(),
            aliases: Vec::new(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let result = engine.recall(RecallRequest {
        query: "Paris travel".to_string(),
        limit: 3,
        deep: true,
    })?;

    let first = result.results.first().expect("expected search results");
    match &first.memory {
        MemoryRecord::Episode(record) => {
            assert_eq!(record.content, "Paris travel checklist for May.")
        }
        other => panic!("expected reranked episode result, got {other:?}"),
    }
    assert!(first
        .reasons
        .iter()
        .any(|reason| matches!(reason, RecallReason::Rerank)));
    Ok(())
}

#[test]
fn ambiguous_query_auto_escalates_to_deep_search() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_rerank(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Paris travel checklist for May.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "place".to_string(),
            name: "Paris".to_string(),
            aliases: Vec::new(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Paris tram maintenance window for May.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "place".to_string(),
            name: "Paris".to_string(),
            aliases: Vec::new(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let result = engine.recall(RecallRequest {
        query: "Paris travel".to_string(),
        limit: 3,
        deep: false,
    })?;

    assert!(result.deep_search_used);
    let first = result.results.first().expect("expected search results");
    match &first.memory {
        MemoryRecord::Episode(record) => {
            assert_eq!(record.content, "Paris travel checklist for May.")
        }
        other => panic!("expected reranked episode result, got {other:?}"),
    }
    assert!(first
        .reasons
        .iter()
        .any(|reason| matches!(reason, RecallReason::Rerank)));
    Ok(())
}

#[test]
fn deep_recall_keeps_results_when_rerank_provider_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_failing_rerank(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Paris travel checklist for May.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "place".to_string(),
            name: "Paris".to_string(),
            aliases: Vec::new(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let result = engine.recall(RecallRequest {
        query: "Paris travel".to_string(),
        limit: 3,
        deep: true,
    })?;

    let first = result.results.first().expect("expected search results");
    match &first.memory {
        MemoryRecord::Episode(record) => {
            assert_eq!(record.content, "Paris travel checklist for May.")
        }
        other => panic!("expected episode result, got {other:?}"),
    }
    assert!(!first
        .reasons
        .iter()
        .any(|reason| matches!(reason, RecallReason::Rerank)));
    Ok(())
}

#[test]
fn dream_merges_provider_extraction_into_memory() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_extraction(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    let report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(report.structured_episodes, 1);

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 5,
        deep: false,
    })?;

    assert!(result.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Entity(entity) if entity.canonical_name == "Alice")
    ));
    assert!(result.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Fact(fact) if fact.predicate == "lives_in" && fact.object_text == "Paris")
    ));
    Ok(())
}

#[test]
fn reaccessed_entity_ranks_ahead_of_stale_alias_peer() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
        content: "Alice handled the launch checklist.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["captain".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: Some(
            chrono::DateTime::parse_from_rfc3339("2024-01-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Bob reviewed the logistics board.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Bob".to_string(),
            aliases: vec!["captain".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-b".to_string()),
        recorded_at: Some(
            chrono::DateTime::parse_from_rfc3339("2024-01-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        confidence: 0.9,
    })?;

    engine.restore(RestoreScope::Text)?;
    let _ = engine.recall(RecallRequest {
        query: "Alice lives in Paris".to_string(),
        limit: 5,
        deep: false,
    })?;

    let result = engine.recall(RecallRequest {
        query: "captain".to_string(),
        limit: 5,
        deep: false,
    })?;

    let first_entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) => Some(entity),
            _ => None,
        })
        .expect("expected entity result");

    assert_eq!(first_entity.canonical_name, "Alice");
    Ok(())
}

#[test]
fn reaccessed_episode_ranks_ahead_of_stale_peer() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    let alpha_id = engine.remember(EpisodeInput {
        content: "warehouse memo alpha".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: Some(
            chrono::DateTime::parse_from_rfc3339("2024-01-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        confidence: 0.9,
    })?;
    let beta_id = engine.remember(EpisodeInput {
        content: "warehouse memo beta".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-b".to_string()),
        recorded_at: Some(
            chrono::DateTime::parse_from_rfc3339("2024-01-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        confidence: 0.9,
    })?;

    let _ = beta_id;
    engine.restore(RestoreScope::Text)?;
    let _ = engine.recall(RecallRequest {
        query: "warehouse memo alpha".to_string(),
        limit: 1,
        deep: false,
    })?;

    let result = engine.recall(RecallRequest {
        query: "warehouse memo".to_string(),
        limit: 5,
        deep: false,
    })?;

    let first_episode = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Episode(record) => Some(record),
            _ => None,
        })
        .expect("expected episode result");

    assert_eq!(first_episode.id, alpha_id);
    Ok(())
}

#[test]
fn dream_archives_duplicates_and_promotes_hot_memory() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let first = engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    let second = engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let first_report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(first_report.archived_records, 1);
    assert!(first_report.promoted_to_l2 >= 1);

    let first_record = engine.reflect(&first)?;
    let second_record = engine.reflect(&second)?;
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
        other => panic!("unexpected duplicate dream state: {other:?}"),
    };

    match engine.reflect(&archived_id)? {
        MemoryRecord::Episode(record) => assert!(record.archived_at.is_some()),
        other => panic!("expected archived episode, got {other:?}"),
    }

    for _ in 0..2 {
        let _ = engine.recall(RecallRequest {
            query: "Alice likes jasmine tea.".to_string(),
            limit: 1,
            deep: false,
        })?;
    }

    let second_report = engine.dream(DreamTrigger::Manual)?;
    assert!(second_report.promoted_to_l3 >= 1);

    match engine.reflect(&survivor_id)? {
        MemoryRecord::Episode(record) => assert_eq!(record.layer, MemoryLayer::L3),
        other => panic!("expected surviving episode, got {other:?}"),
    }

    Ok(())
}

#[test]
fn recall_ignores_archived_episode_from_stale_text_index_until_restore_runs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let first = engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    let second = engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let report = engine.dream_full(DreamTrigger::Manual)?;
    assert_eq!(report.archived_records, 1);

    let first_record = engine.reflect(&first)?;
    let second_record = engine.reflect(&second)?;
    let archived_id = match (&first_record, &second_record) {
        (MemoryRecord::Episode(first_record), MemoryRecord::Episode(_))
            if first_record.archived_at.is_some() =>
        {
            first_record.id.clone()
        }
        (MemoryRecord::Episode(_), MemoryRecord::Episode(second_record))
            if second_record.archived_at.is_some() =>
        {
            second_record.id.clone()
        }
        other => panic!("unexpected duplicate dream state: {other:?}"),
    };

    let result = engine.recall(RecallRequest {
        query: "Alice likes jasmine tea.".to_string(),
        limit: 10,
        deep: false,
    })?;

    assert!(!result.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == archived_id)
    ));
    assert_eq!(
        result
            .results
            .iter()
            .filter(|item| matches!(&item.memory, MemoryRecord::Episode(record) if record.content == "Alice likes jasmine tea."))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn dream_backfills_missing_mentions_from_fact_entities() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
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
            confidence: 0.9,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    conn.execute(
        "DELETE FROM mentions WHERE episode_id = ?1",
        rusqlite::params![episode_id],
    )?;

    let _ = engine.dream(DreamTrigger::Manual)?;

    let mention_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM mentions WHERE episode_id = ?1",
        rusqlite::params![episode_id],
        |row| row.get(0),
    )?;
    assert_eq!(mention_count, 2);
    Ok(())
}

#[test]
fn dream_promotes_related_entities_and_facts_to_l2() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![
            EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
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
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    engine.restore(RestoreScope::Text)?;
    let _ = engine.recall(RecallRequest {
        query: "Alice lives in Paris".to_string(),
        limit: 5,
        deep: false,
    })?;

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.promoted_to_l2 >= 3);

    match engine.reflect(&episode_id)? {
        MemoryRecord::Episode(record) => assert_eq!(record.layer, MemoryLayer::L2),
        other => panic!("expected episode record, got {other:?}"),
    }

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find(|item| matches!(&item.memory, MemoryRecord::Entity(entity) if entity.canonical_name == "Alice"))
        .expect("expected Alice entity after dream");
    match &entity.memory {
        MemoryRecord::Entity(record) => assert_eq!(record.layer, MemoryLayer::L2),
        other => panic!("expected entity record, got {other:?}"),
    }

    let fact = result
        .results
        .iter()
        .find(
            |item| matches!(&item.memory, MemoryRecord::Fact(fact) if fact.predicate == "lives_in"),
        )
        .expect("expected fact after dream");
    match &fact.memory {
        MemoryRecord::Fact(record) => assert_eq!(record.layer, MemoryLayer::L2),
        other => panic!("expected fact record, got {other:?}"),
    }

    Ok(())
}

#[test]
fn dream_promotes_repeated_entity_support_to_l2_without_query_heat() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
        content: "Alice joined the design review today.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice sent the updated roadmap tonight.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-b".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.promoted_to_l2 >= 1);

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after support-based dream");

    assert_eq!(entity.layer, MemoryLayer::L2);
    Ok(())
}

#[test]
fn dream_archives_duplicate_related_facts_and_edges() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![
            EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
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
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![
            EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
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
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.archived_records >= 3);

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let fact_count = result
        .results
        .iter()
        .filter(
            |item| matches!(&item.memory, MemoryRecord::Fact(fact) if fact.predicate == "lives_in"),
        )
        .count();
    let edge_count = result
        .results
        .iter()
        .filter(
            |item| matches!(&item.memory, MemoryRecord::Edge(edge) if edge.predicate == "lives_in"),
        )
        .count();

    assert_eq!(fact_count, 1);
    assert_eq!(edge_count, 1);
    Ok(())
}

#[test]
fn dream_does_not_promote_same_session_entity_support_to_l2() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
        content: "Alice joined the design review today.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice sent the updated roadmap tonight.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        facts: Vec::new(),
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let _ = engine.dream(DreamTrigger::Manual)?;
    engine.restore(RestoreScope::Text)?;

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after same-session dream");

    assert_eq!(entity.layer, MemoryLayer::L1);
    Ok(())
}

#[test]
fn dream_invalidates_conflicting_facts_and_edges() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
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
            confidence: 0.7,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice still lives in Paris.".to_string(),
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
            confidence: 0.72,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-a-2".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let paris_fact_id: String = conn.query_row(
        "SELECT id FROM facts
         WHERE predicate = 'lives_in' AND object_text = 'Paris'
         ORDER BY confidence DESC
         LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    engine.remember(EpisodeInput {
        content: "Alice lives in London.".to_string(),
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
                name: "London".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
        ],
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "London".to_string(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-b".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice is based in London.".to_string(),
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
                name: "London".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
        ],
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "London".to_string(),
            confidence: 0.96,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-b-2".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.invalidated_records >= 2);

    let invalidated_fact = match engine.reflect(&paris_fact_id)? {
        MemoryRecord::Fact(fact) => fact,
        other => panic!("expected fact record, got {other:?}"),
    };
    assert!(invalidated_fact.invalidated_at.is_some());
    assert!(invalidated_fact.valid_from.is_some());
    assert!(invalidated_fact.valid_to.is_some());
    assert!(invalidated_fact.valid_from <= invalidated_fact.valid_to);

    let live_facts = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT object_text
             FROM facts
             WHERE predicate = 'lives_in'
               AND archived_at IS NULL
               AND invalidated_at IS NULL
             ORDER BY object_text",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(live_facts, vec!["London".to_string()]);

    let (active_edge_count, london_valid_from, london_valid_to): (i64, Option<i64>, Option<i64>) =
        conn.query_row(
            "SELECT COUNT(*), MIN(edges.valid_from), MAX(edges.valid_to)
             FROM edges
             JOIN entities object ON object.id = edges.object_entity_id
             WHERE edges.predicate = 'lives_in'
               AND object.canonical_name = 'London'
               AND edges.archived_at IS NULL
               AND edges.invalidated_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    assert!(active_edge_count >= 1);
    assert!(london_valid_from.is_some());
    assert!(london_valid_to.is_none());

    let (london_valid_from, london_valid_to): (Option<i64>, Option<i64>) = conn.query_row(
        "SELECT valid_from, valid_to
         FROM facts
         WHERE predicate = 'lives_in'
           AND object_text = 'London'
           AND archived_at IS NULL
           AND invalidated_at IS NULL
         LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert!(london_valid_from.is_some());
    assert!(london_valid_to.is_none());
    Ok(())
}

#[test]
fn conflicting_edge_keeps_validity_window_when_invalidated() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
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
            confidence: 0.7,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice still lives in Paris.".to_string(),
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
            confidence: 0.72,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-a-2".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let paris_edge_id: String = conn.query_row(
        "SELECT edges.id
         FROM edges
         JOIN entities object ON object.id = edges.object_entity_id
         WHERE edges.predicate = 'lives_in'
           AND object.canonical_name = 'Paris'
         LIMIT 1",
        [],
        |row| row.get(0),
    )?;

    engine.remember(EpisodeInput {
        content: "Alice lives in London.".to_string(),
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
                name: "London".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
        ],
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "London".to_string(),
            confidence: 0.95,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice is based in London.".to_string(),
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
                name: "London".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
        ],
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "London".to_string(),
            confidence: 0.96,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-b-2".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let _ = engine.dream(DreamTrigger::Manual)?;

    let invalidated_paris_edge = match engine.reflect(&paris_edge_id)? {
        MemoryRecord::Edge(edge) => edge,
        other => panic!("expected edge record, got {other:?}"),
    };

    assert!(invalidated_paris_edge.invalidated_at.is_some());
    assert!(invalidated_paris_edge.valid_to.is_some());
    assert!(invalidated_paris_edge.valid_from.is_some());
    assert!(
        invalidated_paris_edge.valid_from <= invalidated_paris_edge.valid_to,
        "expected edge validity window to close in order"
    );

    let (active_london_valid_from, active_london_valid_to): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT edges.valid_from, edges.valid_to
             FROM edges
             JOIN entities object ON object.id = edges.object_entity_id
             WHERE edges.predicate = 'lives_in'
               AND object.canonical_name = 'London'
               AND edges.archived_at IS NULL
               AND edges.invalidated_at IS NULL
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

    assert!(active_london_valid_from.is_some());
    assert!(active_london_valid_to.is_none());
    Ok(())
}

#[test]
fn dream_promotes_repeated_fact_support_to_l3_and_archives_duplicates() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, confidence, session_id, recorded_at) in [
        (
            "Alice currently lives in Paris.",
            0.8,
            "session-a",
            "2026-04-20T09:00:00Z",
        ),
        (
            "Alice has been based in Paris for years.",
            0.95,
            "session-b",
            "2026-04-22T09:00:00Z",
        ),
        (
            "Alice still keeps her home in Paris.",
            0.9,
            "session-c",
            "2026-04-24T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
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
                confidence,
                source: ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.promoted_to_l3 >= 1);
    assert!(report.archived_records >= 4);

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let facts = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => Some(fact),
            _ => None,
        })
        .collect::<Vec<_>>();
    let edges = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Edge(edge) if edge.predicate == "lives_in" => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].object_text, "Paris");
    assert_eq!(facts[0].layer, MemoryLayer::L3);
    assert_eq!(edges.len(), 1);
    Ok(())
}

#[test]
fn dream_does_not_promote_same_session_fact_support_to_l3() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
        content: "Alice currently lives in Paris.".to_string(),
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
            confidence: 0.8,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice has been based in Paris for years.".to_string(),
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
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let facts = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => Some(fact),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(!facts.is_empty());
    assert!(facts.iter().all(|fact| fact.layer != MemoryLayer::L3));
    Ok(())
}

#[test]
fn dream_requires_three_sessions_for_fact_l3_promotion() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice currently lives in Paris.",
            "session-a",
            "2026-04-20T09:00:00Z",
        ),
        (
            "Alice has been based in Paris for years.",
            "session-b",
            "2026-01-03T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
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
                confidence: 0.9,
                source: ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let facts = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => Some(fact),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(!facts.is_empty());
    assert!(facts.iter().all(|fact| fact.layer == MemoryLayer::L2));
    Ok(())
}

#[test]
fn dream_requires_fact_support_span_for_l3_promotion() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice currently lives in Paris.",
            "session-a",
            "2026-01-01T09:00:00Z",
        ),
        (
            "Alice has been based in Paris for years.",
            "session-b",
            "2026-01-01T12:00:00Z",
        ),
        (
            "Alice still keeps her home in Paris.",
            "session-c",
            "2026-01-01T18:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
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
                confidence: 0.9,
                source: ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let facts = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => Some(fact),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(!facts.is_empty());
    assert!(facts.iter().all(|fact| fact.layer == MemoryLayer::L2));
    Ok(())
}

#[test]
fn remember_propagates_recorded_at_to_structured_memory_timestamps() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let recorded_at =
        chrono::DateTime::parse_from_rfc3339("2024-01-15T08:30:00Z")?.with_timezone(&chrono::Utc);

    engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: vec![
            EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
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
            confidence: 0.9,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-historical".to_string()),
        recorded_at: Some(recorded_at),
        confidence: 0.9,
    })?;

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: true,
    })?;

    let alice_entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity");
    assert_eq!(alice_entity.created_at, recorded_at);
    assert_eq!(alice_entity.updated_at, recorded_at);

    let lives_in_fact = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => Some(fact),
            _ => None,
        })
        .expect("expected lives_in fact");
    assert_eq!(lives_in_fact.created_at, recorded_at);
    assert_eq!(lives_in_fact.updated_at, recorded_at);
    assert_eq!(lives_in_fact.valid_from, Some(recorded_at));

    let lives_in_edge = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Edge(edge) if edge.predicate == "lives_in" => Some(edge),
            _ => None,
        })
        .expect("expected lives_in edge");
    assert_eq!(lives_in_edge.created_at, recorded_at);
    assert_eq!(lives_in_edge.updated_at, recorded_at);
    assert_eq!(lives_in_edge.valid_from, Some(recorded_at));
    Ok(())
}

#[test]
fn dream_requires_entity_support_span_for_l3_promotion() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice joined the design review this morning.",
            "session-a",
            "2026-01-01T09:00:00Z",
        ),
        (
            "Alice shared the updated roadmap at noon.",
            "session-b",
            "2026-01-01T12:00:00Z",
        ),
        (
            "Alice followed up with final notes tonight.",
            "session-c",
            "2026-01-01T18:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
            layer: MemoryLayer::L1,
            entities: vec![EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
                confidence: 0.95,
                source: ExtractionSource::Manual,
            }],
            facts: Vec::new(),
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after repeated support dream");

    assert_eq!(entity.layer, MemoryLayer::L2);
    Ok(())
}

#[test]
fn dream_cools_stale_l3_entity_support_back_to_l2() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice joined the design review today.",
            "session-a",
            "2024-01-01T09:00:00Z",
        ),
        (
            "Alice sent the updated roadmap tonight.",
            "session-b",
            "2024-01-02T09:00:00Z",
        ),
        (
            "Alice followed up with final notes this morning.",
            "session-c",
            "2024-01-03T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
            layer: MemoryLayer::L1,
            entities: vec![EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
                confidence: 0.95,
                source: ExtractionSource::Manual,
            }],
            facts: Vec::new(),
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.downgraded_records >= 1);

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after stale dream");

    assert_eq!(entity.layer, MemoryLayer::L2);
    Ok(())
}

#[test]
fn dream_keeps_reaccessed_entity_support_in_l3() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice joined the design review today.",
            "session-a",
            "2024-01-01T09:00:00Z",
        ),
        (
            "Alice sent the updated roadmap tonight.",
            "session-b",
            "2024-01-02T09:00:00Z",
        ),
        (
            "Alice followed up with final notes this morning.",
            "session-c",
            "2024-01-03T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
            layer: MemoryLayer::L1,
            entities: vec![EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
                confidence: 0.95,
                source: ExtractionSource::Manual,
            }],
            facts: Vec::new(),
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let first_report = engine.dream(DreamTrigger::Manual)?;
    assert!(first_report.downgraded_records >= 1);
    engine.restore(RestoreScope::Text)?;

    let warmed = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;
    let warmed_entity = warmed
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after warm query");
    assert_eq!(warmed_entity.layer, MemoryLayer::L2);

    let second_report = engine.dream(DreamTrigger::Manual)?;
    assert!(second_report.promoted_to_l3 >= 1);
    assert_eq!(second_report.downgraded_records, 0);

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after reheated dream");

    assert_eq!(entity.layer, MemoryLayer::L3);
    Ok(())
}

#[test]
fn dream_preserves_fact_observation_timestamp_when_promoting_layers() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let recorded_at =
        chrono::DateTime::parse_from_rfc3339("2024-01-01T09:00:00Z")?.with_timezone(&chrono::Utc);

    for session_id in ["session-a", "session-b", "session-c"] {
        engine.remember(EpisodeInput {
            content: "Alice still lives in Paris.".to_string(),
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
                confidence: 0.9,
                source: ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(recorded_at),
            confidence: 0.9,
        })?;
    }

    let _ = engine.dream(DreamTrigger::Manual)?;

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let (created_at, updated_at, valid_from): (i64, i64, Option<i64>) = conn.query_row(
        "SELECT created_at, updated_at, valid_from
         FROM facts
         WHERE subject_text = 'Alice'
           AND predicate = 'lives_in'
           AND object_text = 'Paris'
           AND archived_at IS NULL
           AND invalidated_at IS NULL
         LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    assert_eq!(
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(created_at)
            .expect("created_at should be a valid millis timestamp"),
        recorded_at
    );
    assert_eq!(
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated_at)
            .expect("updated_at should be a valid millis timestamp"),
        recorded_at
    );
    assert_eq!(
        valid_from
            .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
            .expect("valid_from should be present and valid"),
        recorded_at
    );
    Ok(())
}

#[test]
fn dream_cools_stale_l3_fact_support_back_to_l2() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice currently lives in Paris.",
            "session-a",
            "2024-01-01T09:00:00Z",
        ),
        (
            "Alice has been based in Paris for years.",
            "session-b",
            "2024-01-02T09:00:00Z",
        ),
        (
            "Alice still keeps her home in Paris.",
            "session-c",
            "2024-01-03T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
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
                confidence: 0.9,
                source: ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.downgraded_records >= 1);

    let result = engine.recall(RecallRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let fact = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Fact(fact)
                if fact.subject_text == "Alice"
                    && fact.predicate == "lives_in"
                    && fact.object_text == "Paris" =>
            {
                Some(fact)
            }
            _ => None,
        })
        .expect("expected Alice lives_in Paris fact after stale dream");

    assert_eq!(fact.layer, MemoryLayer::L2);
    Ok(())
}

#[test]
fn dream_keeps_reaccessed_fact_support_in_l3() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice currently lives in Paris.",
            "session-a",
            "2024-01-01T09:00:00Z",
        ),
        (
            "Alice has been based in Paris for years.",
            "session-b",
            "2024-01-02T09:00:00Z",
        ),
        (
            "Alice still keeps her home in Paris.",
            "session-c",
            "2024-01-03T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
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
                confidence: 0.9,
                source: ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let first_report = engine.dream(DreamTrigger::Manual)?;
    assert!(first_report.downgraded_records >= 1);
    engine.restore(RestoreScope::Text)?;

    let _ = engine.recall(RecallRequest {
        query: "Alice Paris".to_string(),
        limit: 20,
        deep: false,
    })?;
    let warmed = engine.recall(RecallRequest {
        query: "Alice Paris".to_string(),
        limit: 20,
        deep: false,
    })?;
    let warmed_fact = warmed
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Fact(fact)
                if fact.subject_text == "Alice"
                    && fact.predicate == "lives_in"
                    && fact.object_text == "Paris" =>
            {
                Some(fact)
            }
            _ => None,
        })
        .expect("expected Alice lives_in Paris fact after warm query");
    assert_eq!(warmed_fact.layer, MemoryLayer::L2);

    let second_report = engine.dream(DreamTrigger::Manual)?;
    assert!(second_report.promoted_to_l3 >= 1);
    assert_eq!(second_report.downgraded_records, 0);

    let result = engine.recall(RecallRequest {
        query: "Alice Paris".to_string(),
        limit: 20,
        deep: false,
    })?;

    let fact = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Fact(fact)
                if fact.subject_text == "Alice"
                    && fact.predicate == "lives_in"
                    && fact.object_text == "Paris" =>
            {
                Some(fact)
            }
            _ => None,
        })
        .expect("expected Alice lives_in Paris fact after reheated dream");

    assert_eq!(fact.layer, MemoryLayer::L3);
    Ok(())
}

#[test]
fn dream_refreshes_l3_cache_after_cooling_entity_support() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice joined the design review today.",
            "session-a",
            "2024-01-01T09:00:00Z",
        ),
        (
            "Alice sent the updated roadmap tonight.",
            "session-b",
            "2024-01-02T09:00:00Z",
        ),
        (
            "Alice followed up with final notes this morning.",
            "session-c",
            "2024-01-03T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
            layer: MemoryLayer::L1,
            entities: vec![EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
                confidence: 0.95,
                source: ExtractionSource::Manual,
            }],
            facts: Vec::new(),
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let result = engine.dream(DreamTrigger::Manual)?;
    assert!(result.downgraded_records >= 1);
    assert_eq!(engine.state()?.l3_cached, 0);

    let query = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = query
        .results
        .iter()
        .find(|item| matches!(&item.memory, MemoryRecord::Entity(entity) if entity.canonical_name == "Alice"))
        .expect("expected Alice entity after cooling");

    assert!(!entity
        .reasons
        .iter()
        .any(|reason| matches!(reason, RecallReason::L3)));
    Ok(())
}

#[test]
fn dream_promotes_repeated_entity_support_to_l3_without_query_heat() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id, recorded_at) in [
        (
            "Alice joined the design review today.",
            "session-a",
            "2026-01-01T09:00:00Z",
        ),
        (
            "Alice sent the updated roadmap tonight.",
            "session-b",
            "2026-04-22T09:00:00Z",
        ),
        (
            "Alice followed up with final notes this morning.",
            "session-c",
            "2026-04-24T09:00:00Z",
        ),
    ] {
        engine.remember(EpisodeInput {
            content: content.to_string(),
            layer: MemoryLayer::L1,
            entities: vec![EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
                confidence: 0.95,
                source: ExtractionSource::Manual,
            }],
            facts: Vec::new(),
            source_episode_id: None,
            session_id: Some(session_id.to_string()),
            recorded_at: Some(
                chrono::DateTime::parse_from_rfc3339(recorded_at)?.with_timezone(&chrono::Utc),
            ),
            confidence: 0.9,
        })?;
    }

    let report = engine.dream(DreamTrigger::Manual)?;
    assert!(report.promoted_to_l3 >= 1);

    let result = engine.recall(RecallRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Entity(entity) if entity.canonical_name == "Alice" => Some(entity),
            _ => None,
        })
        .expect("expected Alice entity after repeated cross-session dream");

    assert_eq!(entity.layer, MemoryLayer::L3);
    Ok(())
}

#[test]
fn full_dream_runs_extra_stabilization_pass() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    let episode_id = engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream_full(DreamTrigger::Manual)?;
    assert_eq!(report.passes_run, 2);

    let state = engine.state()?;
    assert_eq!(state.layers.l1, 0);
    assert_eq!(state.layers.l2, 1);
    assert_eq!(state.layers.l3, 0);

    match engine.reflect(&episode_id)? {
        MemoryRecord::Episode(record) => assert_eq!(record.layer, MemoryLayer::L2),
        other => panic!("expected episode record, got {other:?}"),
    }

    Ok(())
}

#[test]
fn full_dream_stops_after_first_pass_when_memory_is_stable() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    let episode_id = engine.remember(EpisodeInput {
        content: "Alice brewed a fresh cup of jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream_full(DreamTrigger::Manual)?;
    assert_eq!(report.passes_run, 1);
    assert_eq!(report.promoted_to_l2, 0);
    assert_eq!(report.promoted_to_l3, 0);

    let state = engine.state()?;
    assert_eq!(state.layers.l1, 1);
    assert_eq!(state.layers.l2, 0);
    assert_eq!(state.layers.l3, 0);

    match engine.reflect(&episode_id)? {
        MemoryRecord::Episode(record) => assert_eq!(record.layer, MemoryLayer::L1),
        other => panic!("expected episode record, got {other:?}"),
    }

    Ok(())
}

#[test]
fn remember_marks_text_index_pending_until_restore_runs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "Riverbank Robotics builds warehouse drones.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let pending_stats = engine.state()?;
    assert_eq!(pending_stats.text_index.status, "pending");
    assert_eq!(pending_stats.text_index.pending_updates, 1);
    assert_eq!(pending_stats.text_index.failed_updates, 0);
    assert_eq!(
        pending_stats.text_index.detail.as_deref(),
        Some("pending restore after queued updates")
    );

    let before_refresh = engine.recall(RecallRequest {
        query: "warehouse drones".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(!before_refresh.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Bm25))
    ));

    let refresh = engine.restore(RestoreScope::Text)?;
    assert_eq!(refresh.text_documents, 1);

    let ready_stats = engine.state()?;
    assert_eq!(ready_stats.text_index.status, "ready");
    assert_eq!(ready_stats.text_index.pending_updates, 0);
    assert_eq!(ready_stats.text_index.failed_updates, 0);

    let after_refresh = engine.recall(RecallRequest {
        query: "warehouse drones".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(after_refresh.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Bm25))
    ));
    Ok(())
}

#[test]
fn remember_marks_vector_index_pending_until_restore_runs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_vectors(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "我今天很高兴".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let pending_stats = engine.state()?;
    assert_eq!(pending_stats.vector_index.status, "pending");
    assert_eq!(pending_stats.vector_index.pending_updates, 1);
    assert_eq!(pending_stats.vector_index.failed_updates, 0);
    assert_eq!(
        pending_stats.vector_index.detail.as_deref(),
        Some("pending restore after queued updates")
    );

    let before_refresh = engine.recall(RecallRequest {
        query: "开心".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(!before_refresh.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Vector))
    ));

    let refresh = engine.restore(RestoreScope::Vector)?;
    assert_eq!(refresh.vector_documents, 1);

    let ready_stats = engine.state()?;
    assert_eq!(ready_stats.vector_index.status, "ready");
    assert_eq!(ready_stats.vector_index.pending_updates, 0);
    assert_eq!(ready_stats.vector_index.failed_updates, 0);

    let after_refresh = engine.recall(RecallRequest {
        query: "开心".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(after_refresh.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Vector))
    ));
    Ok(())
}

#[test]
fn remember_keeps_truth_source_write_when_embedding_provider_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_failing_embeddings(temp.path())?;

    let episode_id = engine.remember(EpisodeInput {
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
            confidence: 0.9,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let state = engine.state()?;
    assert_eq!(state.text_index.status, "pending");
    assert_eq!(state.vector_index.pending_updates, 0);

    match engine.reflect(&episode_id)? {
        MemoryRecord::Episode(record) => {
            assert_eq!(record.content, "Alice lives in Paris.")
        }
        other => panic!("expected episode record, got {other:?}"),
    }

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let fact_count: i64 = conn.query_row("SELECT COUNT(*) FROM facts", [], |row| row.get(0))?;
    let entity_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
    assert_eq!(fact_count, 1);
    assert_eq!(entity_count, 2);
    Ok(())
}

#[test]
fn remember_accumulates_pending_text_updates_until_restore_runs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
        content: "Episode one".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Episode two".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let pending = engine.state()?;
    assert_eq!(pending.text_index.status, "pending");
    assert_eq!(pending.text_index.pending_updates, 2);
    assert_eq!(pending.text_index.failed_updates, 0);

    let report = engine.restore(RestoreScope::Text)?;
    assert_eq!(report.text_documents, 2);

    let ready = engine.state()?;
    assert_eq!(ready.text_index.status, "ready");
    assert_eq!(ready.text_index.pending_updates, 0);
    assert_eq!(ready.text_index.failed_updates, 0);

    Ok(())
}

#[test]
fn reopening_engine_preserves_pending_text_updates_until_restore_runs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.remember(EpisodeInput {
        content: "Reopen should not rebuild indexes automatically.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    drop(engine);

    let reopened = open_engine(temp.path())?;
    let pending = reopened.state()?;
    assert_eq!(pending.text_index.status, "pending");
    assert_eq!(pending.text_index.pending_updates, 1);

    let before_restore = reopened.recall(RecallRequest {
        query: "rebuild indexes automatically".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(!before_restore.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Bm25))
    ));

    reopened.restore(RestoreScope::Text)?;
    let after_restore = reopened.recall(RecallRequest {
        query: "rebuild indexes automatically".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(after_restore.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Bm25))
    ));

    Ok(())
}

#[test]
fn dream_marks_text_index_pending_after_memory_lifecycle_changes() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.remember(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    engine.restore(RestoreScope::Text)?;

    let ready = engine.state()?;
    assert_eq!(ready.text_index.status, "ready");
    assert_eq!(ready.text_index.pending_updates, 0);

    let report = engine.dream_full(DreamTrigger::Manual)?;
    assert_eq!(report.passes_run, 2);

    let pending = engine.state()?;
    assert_eq!(pending.text_index.status, "pending");
    assert!(pending.text_index.pending_updates >= 1);

    Ok(())
}

#[test]
fn dream_continues_structuring_when_embeddings_fail() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = MemoryEngine::open(
        EngineConfig::new(temp.path())
            .with_embedding_provider(Arc::new(FailingEmbeddingProvider))
            .with_extraction_provider(Arc::new(TestExtractionProvider)),
    )?;

    let episode_id = engine.remember(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.dream(DreamTrigger::Manual)?;
    assert_eq!(report.structured_episodes, 1);
    assert_eq!(report.structured_entities, 2);
    assert_eq!(report.structured_facts, 1);
    assert_eq!(report.extraction_failures, 0);

    let conn = Connection::open(temp.path().join("memory.db"))?;
    let structured: i64 = conn.query_row(
        "SELECT structured_at IS NOT NULL FROM episodes WHERE id = ?1",
        rusqlite::params![episode_id],
        |row| row.get(0),
    )?;
    let fact_count: i64 = conn.query_row("SELECT COUNT(*) FROM facts", [], |row| row.get(0))?;
    let entity_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
    assert_eq!(structured, 1);
    assert_eq!(fact_count, 1);
    assert_eq!(entity_count, 2);

    let state = engine.state()?;
    assert_eq!(state.vector_index.pending_updates, 0);
    Ok(())
}

#[test]
fn failed_vector_jobs_do_not_block_new_pending_restore_work() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_vectors(temp.path())?;

    engine.remember(EpisodeInput {
        content: "stale broken vector".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    let conn = Connection::open(temp.path().join("memory.db"))?;
    conn.execute(
        "UPDATE episodes SET vector_json = ?2 WHERE content = ?1",
        rusqlite::params!["stale broken vector", "[1.0, 2.0]"],
    )?;
    conn.execute(
        "UPDATE index_jobs
         SET status = 'failed', attempts = 2, last_error = 'vector dimension mismatch'
         WHERE index_name = 'vector'",
        [],
    )?;

    let episode_id = engine.remember(EpisodeInput {
        content: "我今天很高兴".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let report = engine.restore(RestoreScope::Vector)?;
    assert_eq!(report.vector_documents, 1);

    let state = engine.state()?;
    assert_eq!(state.vector_index.status, "failed");
    assert_eq!(state.vector_index.failed_updates, 1);
    assert_eq!(state.vector_index.pending_updates, 0);
    assert_eq!(state.vector_index.failed_attempts_max, 2);

    let result = engine.recall(RecallRequest {
        query: "开心".to_string(),
        limit: 3,
        deep: false,
    })?;
    assert!(result.results.iter().any(
        |item| matches!(&item.memory, MemoryRecord::Episode(record) if record.id == episode_id)
            && item
                .reasons
                .iter()
                .any(|reason| matches!(reason, RecallReason::Vector))
    ));

    Ok(())
}
