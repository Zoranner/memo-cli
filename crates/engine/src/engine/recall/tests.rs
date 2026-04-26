use chrono::{TimeZone, Utc};

use crate::types::{EpisodeRecord, MemoryLayer, MemoryRecord, RecallResult, RecallResultSet};

use super::{session_cache::trim_session_cache, strategy::should_auto_escalate_to_deep_search};
use crate::engine::SessionCache;

fn episode_result(score: f32, reasons: Vec<crate::types::RecallReason>) -> RecallResult {
    RecallResult {
        memory: MemoryRecord::Episode(EpisodeRecord {
            id: "episode-1".to_string(),
            content: "Paris travel checklist for May.".to_string(),
            layer: MemoryLayer::L1,
            confidence: 0.9,
            source_episode_id: None,
            session_id: None,
            created_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
            last_seen_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
            archived_at: None,
            invalidated_at: None,
            hit_count: 0,
        }),
        score,
        reasons,
    }
}

#[test]
fn weak_single_candidate_can_trigger_deep_search() {
    let result = RecallResultSet {
        deep_search_used: false,
        total_candidates: 1,
        results: vec![episode_result(0.72, vec![crate::types::RecallReason::Bm25])],
    };

    assert!(should_auto_escalate_to_deep_search(&result));
}

#[test]
fn decisive_exact_hit_stays_on_fast_path() {
    let result = RecallResultSet {
        deep_search_used: false,
        total_candidates: 1,
        results: vec![episode_result(3.2, vec![crate::types::RecallReason::Exact])],
    };

    assert!(!should_auto_escalate_to_deep_search(&result));
}

#[test]
fn trim_session_cache_caps_recent_topics_and_memory_ids() {
    let mut session = SessionCache {
        recent_memory_ids: (0..130).map(|index| format!("memory-{index}")).collect(),
        recent_topics: (0..70).map(|index| format!("topic-{index}")).collect(),
        ..Default::default()
    };

    trim_session_cache(&mut session);

    assert_eq!(session.recent_memory_ids.len(), 66);
    assert_eq!(session.recent_topics.len(), 38);
    assert_eq!(
        session.recent_memory_ids.first().map(String::as_str),
        Some("memory-64")
    );
    assert_eq!(
        session.recent_topics.first().map(String::as_str),
        Some("topic-32")
    );
}
