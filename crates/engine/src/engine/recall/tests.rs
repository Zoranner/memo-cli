use chrono::{TimeZone, Utc};

use crate::types::{
    EpisodeRecord, MemoryLayer, MemoryRecord, RecallCapabilities, RecallResult, RecallResultSet,
};

use super::ranking::{pinned_boost, query_coverage, query_subject_tokens};
use super::{session_cache::trim_session_cache, strategy::should_auto_escalate_to_deep_search};
use crate::engine::SessionCache;

fn episode_result(score: f32, reasons: Vec<crate::types::RecallReason>) -> RecallResult {
    RecallResult {
        memory: episode_record("episode-1", "Paris travel checklist for May."),
        score,
        reasons,
    }
}

fn episode_record(id: &str, content: &str) -> MemoryRecord {
    MemoryRecord::Episode(EpisodeRecord {
        id: id.to_string(),
        content: content.to_string(),
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
    })
}

fn result_set(results: Vec<RecallResult>) -> RecallResultSet {
    RecallResultSet {
        deep_search_used: false,
        total_candidates: results.len(),
        provider_calls: 0,
        capabilities: RecallCapabilities {
            text: results.iter().any(|result| {
                result
                    .reasons
                    .iter()
                    .any(|reason| matches!(reason, crate::types::RecallReason::Bm25))
            }),
            vector: results.iter().any(|result| {
                result
                    .reasons
                    .iter()
                    .any(|reason| matches!(reason, crate::types::RecallReason::Vector))
            }),
            l1: results
                .iter()
                .any(|result| result.memory.layer() == MemoryLayer::L1),
            l2: results
                .iter()
                .any(|result| result.memory.layer() == MemoryLayer::L2),
            l3: results
                .iter()
                .any(|result| result.memory.layer() == MemoryLayer::L3),
            working_set: results.iter().any(|result| {
                result
                    .reasons
                    .iter()
                    .any(|reason| matches!(reason, crate::types::RecallReason::WorkingSet))
            }),
        },
        results,
    }
}

#[test]
fn weak_single_candidate_can_trigger_deep_search() {
    let result = result_set(vec![episode_result(
        0.72,
        vec![crate::types::RecallReason::Bm25],
    )]);

    assert!(should_auto_escalate_to_deep_search(&result));
}

#[test]
fn decisive_exact_hit_stays_on_fast_path() {
    let result = result_set(vec![episode_result(
        3.2,
        vec![crate::types::RecallReason::Exact],
    )]);

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

#[test]
fn query_coverage_counts_chinese_terms() {
    let matching = episode_record("episode-1", "张伟 正在 上海 负责 低空物流 调度");
    let unrelated = episode_record("episode-2", "李雷 正在 深圳 负责 智能仓储 调度");

    assert!(query_coverage("张伟 上海 低空物流", &matching) >= 0.75);
    assert!(query_coverage("张伟 上海 低空物流", &unrelated) < 0.5);
}

#[test]
fn subject_tokens_include_chinese_subjects_without_location_modifiers() {
    let subjects = query_subject_tokens("张伟 现在 在 上海 哪里");

    assert!(subjects.contains("张伟"));
    assert!(!subjects.contains("现在"));
    assert!(!subjects.contains("哪里"));
}

#[test]
fn pinned_boost_requires_meaningful_query_match() {
    let matching = episode_record(
        "episode-1",
        "Riverbank Robotics keeps the fleet telemetry plan.",
    );
    let weak = episode_record(
        "episode-2",
        "Riverbank Robotics keeps shared archive notes.",
    );

    assert!(pinned_boost("fleet telemetry plan", &matching) > 0.0);
    assert_eq!(pinned_boost("fleet telemetry plan", &weak), 0.0);
}
