use std::{path::Path, sync::Arc};

use anyhow::Result;
use memo_engine::{
    ConsolidationTrigger, EmbeddingProvider, EngineConfig, EntityInput, EpisodeInput,
    ExtractedEntity, ExtractedFact, ExtractionProvider, ExtractionResult, ExtractionSource,
    FactInput, MemoryEngine, MemoryLayer, MemoryRecord, RetrieveReason, RetrieveRequest,
};
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

#[test]
fn preview_ingest_merges_provider_and_manual_inputs() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_extraction(temp.path())?;

    let preview = engine.preview_ingest(&EpisodeInput {
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
        session_id: None,
        recorded_at: None,
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
        session_id: None,
        recorded_at: None,
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
        session_id: None,
        recorded_at: None,
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
        session_id: None,
        recorded_at: None,
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
fn ingestion_merges_provider_extraction_into_memory() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine_with_extraction(temp.path())?;
    engine.ingest_episode(EpisodeInput {
        content: "Alice lives in Paris.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let result = engine.query(RetrieveRequest {
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
fn consolidation_archives_duplicates_and_promotes_hot_memory() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let first = engine.ingest_episode(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;
    let second = engine.ingest_episode(EpisodeInput {
        content: "Alice likes jasmine tea.".to_string(),
        layer: MemoryLayer::L1,
        entities: Vec::new(),
        facts: Vec::new(),
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
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

#[test]
fn consolidation_promotes_related_entities_and_facts_to_l2() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    let episode_id = engine.ingest_episode(EpisodeInput {
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

    let _ = engine.query(RetrieveRequest {
        query: "Alice".to_string(),
        limit: 5,
        deep: false,
    })?;

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(report.promoted_to_l2 >= 3);

    match engine.inspect_memory(&episode_id)? {
        MemoryRecord::Episode(record) => assert_eq!(record.layer, MemoryLayer::L2),
        other => panic!("expected episode record, got {other:?}"),
    }

    let result = engine.query(RetrieveRequest {
        query: "Ally".to_string(),
        limit: 10,
        deep: false,
    })?;

    let entity = result
        .results
        .iter()
        .find(|item| matches!(&item.memory, MemoryRecord::Entity(entity) if entity.canonical_name == "Alice"))
        .expect("expected Alice entity after consolidation");
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
        .expect("expected fact after consolidation");
    match &fact.memory {
        MemoryRecord::Fact(record) => assert_eq!(record.layer, MemoryLayer::L2),
        other => panic!("expected fact record, got {other:?}"),
    }

    Ok(())
}

#[test]
fn consolidation_promotes_repeated_entity_support_to_l2_without_query_heat() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.ingest_episode(EpisodeInput {
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
    engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(report.promoted_to_l2 >= 1);

    let result = engine.query(RetrieveRequest {
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
        .expect("expected Alice entity after support-based consolidation");

    assert_eq!(entity.layer, MemoryLayer::L2);
    Ok(())
}

#[test]
fn consolidation_archives_duplicate_related_facts_and_edges() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;
    engine.ingest_episode(EpisodeInput {
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
    engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(report.archived_records >= 3);

    let result = engine.query(RetrieveRequest {
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
fn consolidation_does_not_promote_same_session_entity_support_to_l2() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.ingest_episode(EpisodeInput {
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
    engine.ingest_episode(EpisodeInput {
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

    let _ = engine.consolidate(ConsolidationTrigger::Manual)?;

    let result = engine.query(RetrieveRequest {
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
        .expect("expected Alice entity after same-session consolidation");

    assert_eq!(entity.layer, MemoryLayer::L1);
    Ok(())
}

#[test]
fn consolidation_invalidates_conflicting_facts_and_edges() -> Result<()> {
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
            confidence: 0.7,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: Some("session-a".to_string()),
        recorded_at: None,
        confidence: 0.9,
    })?;

    let paris_fact_id = engine
        .query(RetrieveRequest {
            query: "Paris".to_string(),
            limit: 20,
            deep: true,
        })?
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => Some(fact.id.clone()),
            _ => None,
        })
        .expect("expected Paris fact before consolidation");
    engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(report.invalidated_records >= 2);

    let invalidated_fact = match engine.inspect_memory(&paris_fact_id)? {
        MemoryRecord::Fact(fact) => fact,
        other => panic!("expected fact record, got {other:?}"),
    };
    assert!(invalidated_fact.invalidated_at.is_some());
    assert!(invalidated_fact.valid_from.is_some());
    assert!(invalidated_fact.valid_to.is_some());
    assert!(invalidated_fact.valid_from <= invalidated_fact.valid_to);

    let result = engine.query(RetrieveRequest {
        query: "Alice".to_string(),
        limit: 20,
        deep: false,
    })?;

    let live_facts = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => {
                Some(fact.object_text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let live_edges = result
        .results
        .iter()
        .filter_map(|item| match &item.memory {
            MemoryRecord::Edge(edge) if edge.predicate == "lives_in" => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(live_facts, vec!["London"]);
    assert_eq!(live_edges.len(), 1);
    assert!(live_edges[0].valid_from.is_some());
    assert!(live_edges[0].valid_to.is_none());
    let active_london_fact = result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Fact(fact)
                if fact.predicate == "lives_in" && fact.object_text == "London" =>
            {
                Some(fact)
            }
            _ => None,
        })
        .expect("expected active London fact");
    assert!(active_london_fact.valid_from.is_some());
    assert!(active_london_fact.valid_to.is_none());
    Ok(())
}

#[test]
fn conflicting_edge_keeps_validity_window_when_invalidated() -> Result<()> {
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
            confidence: 0.7,
            source: ExtractionSource::Manual,
        }],
        source_episode_id: None,
        session_id: None,
        recorded_at: None,
        confidence: 0.9,
    })?;

    let paris_edge_id = engine
        .query(RetrieveRequest {
            query: "Paris".to_string(),
            limit: 20,
            deep: true,
        })?
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Edge(edge) if edge.predicate == "lives_in" => Some(edge.id.clone()),
            _ => None,
        })
        .expect("expected Paris edge before consolidation");

    engine.ingest_episode(EpisodeInput {
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

    let _ = engine.consolidate(ConsolidationTrigger::Manual)?;

    let invalidated_paris_edge = match engine.inspect_memory(&paris_edge_id)? {
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

    let london_result = engine.query(RetrieveRequest {
        query: "London".to_string(),
        limit: 20,
        deep: true,
    })?;

    let active_london_edge = london_result
        .results
        .iter()
        .find_map(|item| match &item.memory {
            MemoryRecord::Edge(edge) if edge.predicate == "lives_in" => Some(edge),
            _ => None,
        })
        .expect("expected invalidated Paris edge with validity window");

    assert!(active_london_edge.valid_from.is_some());
    assert!(active_london_edge.valid_to.is_none());
    Ok(())
}

#[test]
fn consolidation_promotes_repeated_fact_support_to_l3_and_archives_duplicates() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, confidence, session_id, recorded_at) in [
        (
            "Alice currently lives in Paris.",
            0.8,
            "session-a",
            "2026-01-01T09:00:00Z",
        ),
        (
            "Alice has been based in Paris for years.",
            0.95,
            "session-b",
            "2026-01-03T09:00:00Z",
        ),
        (
            "Alice still keeps her home in Paris.",
            0.9,
            "session-c",
            "2026-01-05T09:00:00Z",
        ),
    ] {
        engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(report.promoted_to_l3 >= 1);
    assert!(report.archived_records >= 4);

    let result = engine.query(RetrieveRequest {
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
fn consolidation_does_not_promote_same_session_fact_support_to_l3() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    engine.ingest_episode(EpisodeInput {
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
    engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.query(RetrieveRequest {
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
fn consolidation_requires_three_sessions_for_fact_l3_promotion() -> Result<()> {
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
            "2026-01-03T09:00:00Z",
        ),
    ] {
        engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.query(RetrieveRequest {
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
fn consolidation_requires_fact_support_span_for_l3_promotion() -> Result<()> {
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
        engine.ingest_episode(EpisodeInput {
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

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert_eq!(report.promoted_to_l3, 0);

    let result = engine.query(RetrieveRequest {
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
fn consolidation_promotes_repeated_entity_support_to_l3_without_query_heat() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = open_engine(temp.path())?;

    for (content, session_id) in [
        ("Alice joined the design review today.", "session-a"),
        ("Alice sent the updated roadmap tonight.", "session-b"),
        (
            "Alice followed up with final notes this morning.",
            "session-c",
        ),
    ] {
        engine.ingest_episode(EpisodeInput {
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
            recorded_at: None,
            confidence: 0.9,
        })?;
    }

    let report = engine.consolidate(ConsolidationTrigger::Manual)?;
    assert!(report.promoted_to_l3 >= 1);

    let result = engine.query(RetrieveRequest {
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
        .expect("expected Alice entity after repeated cross-session consolidation");

    assert_eq!(entity.layer, MemoryLayer::L3);
    Ok(())
}
