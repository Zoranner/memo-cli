mod common;
mod memory;
mod system;

pub(crate) use common::render_json_or_text;
pub(crate) use memory::{render_recall_result, render_reflection, render_remember_preview};
pub(crate) use system::{
    render_awaken_result, render_dream_report, render_restore_report, render_state,
};

#[cfg(test)]
mod tests {
    use super::{render_dream_report, render_recall_result, render_reflection, render_state};
    use crate::providers::status::{
        ProviderCapabilityStatus, ProviderHealth, ProviderRuntimeSummary,
    };
    use chrono::{TimeZone, Utc};
    use memo_engine::{
        DreamReport, EpisodeRecord, FactRecord, IndexStatus, MemoryLayer, MemoryRecord,
        RecallReason, RecallResult, RecallResultSet, SystemState,
    };

    #[test]
    fn render_state_without_json_uses_human_summary() {
        let output = render_state(
            &SystemState {
                episode_count: 3,
                entity_count: 2,
                fact_count: 1,
                edge_count: 1,
                l3_cached: 4,
                layers: memo_engine::LayerSummary {
                    l1: 2,
                    l2: 1,
                    l3: 0,
                    archived: 3,
                    invalidated: 1,
                },
                text_index: IndexStatus {
                    name: "text".to_string(),
                    doc_count: 8,
                    status: "ready".to_string(),
                    detail: None,
                    pending_updates: 0,
                    failed_updates: 0,
                    failed_attempts_max: 0,
                    last_error: None,
                },
                vector_index: IndexStatus {
                    name: "vector".to_string(),
                    doc_count: 5,
                    status: "failed".to_string(),
                    detail: Some("restore failed for queued updates".to_string()),
                    pending_updates: 0,
                    failed_updates: 2,
                    failed_attempts_max: 3,
                    last_error: Some("vector dimension mismatch".to_string()),
                },
            },
            &ProviderRuntimeSummary {
                statuses: vec![ProviderCapabilityStatus {
                    capability: "embedding".to_string(),
                    provider_ref: "openai.embed".to_string(),
                    status: ProviderHealth::Degraded,
                    consecutive_failures: 2,
                    last_error: Some("rate limit".to_string()),
                    updated_at: Utc.with_ymd_and_hms(2026, 4, 23, 8, 0, 0).unwrap(),
                }],
                read_error: None,
            },
            false,
        )
        .expect("expected human state output");

        assert!(output.contains("State"));
        assert!(output.contains("layers: l1=2 l2=1 l3=0 archived=3 invalidated=1"));
        assert!(output.contains("vector_index: failed docs=5"));
        assert!(output.contains("failed_updates=2"));
        assert!(output.contains("failed_attempts_max=3"));
        assert!(output.contains("last_error=vector dimension mismatch"));
        assert!(output.contains("provider_runtime: embedding=degraded"));
        assert!(output.contains("consecutive_failures=2"));
        assert!(output.contains("last_error=rate limit"));
        assert!(!output.contains("dream_jobs"));
    }

    #[test]
    fn render_recall_without_json_summarizes_results() {
        let output = render_recall_result(
            &RecallResultSet {
                total_candidates: 2,
                deep_search_used: true,
                results: vec![RecallResult {
                    memory: MemoryRecord::Episode(EpisodeRecord {
                        id: "ep-1".to_string(),
                        content: "Alice lives in Paris.".to_string(),
                        layer: MemoryLayer::L2,
                        confidence: 0.9,
                        source_episode_id: None,
                        session_id: None,
                        created_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                        updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                        last_seen_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                        archived_at: None,
                        invalidated_at: None,
                        hit_count: 3,
                    }),
                    score: 3.4,
                    reasons: vec![RecallReason::Alias, RecallReason::LayerBoost],
                }],
            },
            false,
        )
        .expect("expected human recall output");

        assert!(output.contains("Recalled 1 item(s)"));
        assert!(output.contains("[episode:ep-1] score=3.400 layer=L2"));
        assert!(output.contains("reasons: alias, layer_boost"));
    }

    #[test]
    fn render_reflection_marks_archived_episode_status() {
        let output = render_reflection(
            &MemoryRecord::Episode(EpisodeRecord {
                id: "ep-archived".to_string(),
                content: "Alice archived note.".to_string(),
                layer: MemoryLayer::L2,
                confidence: 0.9,
                source_episode_id: None,
                session_id: None,
                created_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 11, 0, 0).unwrap(),
                last_seen_at: Utc.with_ymd_and_hms(2026, 4, 21, 11, 0, 0).unwrap(),
                archived_at: Some(Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap()),
                invalidated_at: None,
                hit_count: 3,
            }),
            false,
        )
        .expect("expected reflection output");

        assert!(output.contains("status: archived"));
        assert!(output.contains("archived_at: 2026-04-21T12:00:00+00:00"));
    }

    #[test]
    fn render_reflection_marks_invalidated_fact_window() {
        let output = render_reflection(
            &MemoryRecord::Fact(FactRecord {
                id: "fact-1".to_string(),
                subject_entity_id: Some("alice".to_string()),
                subject_text: "Alice".to_string(),
                predicate: "lives_in".to_string(),
                object_entity_id: Some("paris".to_string()),
                object_text: "Paris".to_string(),
                layer: MemoryLayer::L2,
                confidence: 0.8,
                source_episode_id: Some("ep-1".to_string()),
                created_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 13, 0, 0).unwrap(),
                valid_from: Some(Utc.with_ymd_and_hms(2026, 4, 20, 9, 0, 0).unwrap()),
                valid_to: Some(Utc.with_ymd_and_hms(2026, 4, 21, 13, 0, 0).unwrap()),
                archived_at: None,
                invalidated_at: Some(Utc.with_ymd_and_hms(2026, 4, 21, 13, 0, 0).unwrap()),
                hit_count: 2,
            }),
            false,
        )
        .expect("expected reflection output");

        assert!(output.contains("status: invalidated"));
        assert!(output.contains("invalidated_at: 2026-04-21T13:00:00+00:00"));
        assert!(output.contains("valid_from: 2026-04-20T09:00:00+00:00"));
        assert!(output.contains("valid_to: 2026-04-21T13:00:00+00:00"));
    }

    #[test]
    fn render_state_with_json_returns_json() {
        let output = render_state(
            &SystemState {
                text_index: IndexStatus {
                    name: "text".to_string(),
                    ..Default::default()
                },
                vector_index: IndexStatus {
                    name: "vector".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            &ProviderRuntimeSummary::default(),
            true,
        )
        .expect("expected json state output");

        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("expected valid json output");
        assert!(parsed.get("dream_jobs").is_none());
        assert_eq!(parsed["state"]["layers"]["l1"], 0);
        assert_eq!(parsed["state"]["layers"]["archived"], 0);
        assert!(parsed["provider_runtime"]["statuses"]
            .as_array()
            .expect("expected provider_runtime statuses array")
            .is_empty());
    }

    #[test]
    fn render_full_dream_report_uses_pass_count() {
        let output = render_dream_report(
            &DreamReport {
                passes_run: 2,
                promoted_to_l2: 3,
                promoted_to_l3: 1,
                downgraded_records: 0,
                archived_records: 2,
                invalidated_records: 1,
                ..Default::default()
            },
            true,
            false,
        )
        .expect("expected human dream output");

        assert!(output.contains("Dream (full) complete"));
        assert!(output.contains("passes_run: 2"));
    }
}
