use anyhow::Result;
use chrono::{DateTime, Utc};
use memo_engine::{MemoryRecord, RecallReason, RecallResultSet, RememberPreview};

use super::common::render_json_or_text;

pub(crate) fn render_remember_preview(preview: &RememberPreview, json: bool) -> Result<String> {
    let human = format!(
        "Remember preview\ncontent: {}\nlayer: {}\nentities: {}\nfacts: {}",
        preview.content,
        preview.layer.as_str(),
        preview.entities.len(),
        preview.facts.len(),
    );
    render_json_or_text(preview, &human, json)
}

pub(crate) fn render_recall_result(result: &RecallResultSet, json: bool) -> Result<String> {
    if json {
        return render_json_or_text(result, "", true);
    }

    let mut lines = vec![format!(
        "Recalled {} item(s) from {} candidate(s){}",
        result.results.len(),
        result.total_candidates,
        if result.deep_search_used {
            " with deep recall"
        } else {
            ""
        }
    )];

    for (index, item) in result.results.iter().enumerate() {
        lines.push(format!(
            "{}. [{}:{}] score={:.3} layer={}",
            index + 1,
            item.memory.kind(),
            item.memory.id(),
            item.score,
            item.memory.layer().as_str(),
        ));
        lines.push(format!("   {}", memory_summary(&item.memory)));
        if !item.reasons.is_empty() {
            lines.push(format!(
                "   reasons: {}",
                item.reasons
                    .iter()
                    .map(recall_reason_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    Ok(lines.join("\n"))
}

pub(crate) fn render_reflection(record: &MemoryRecord, json: bool) -> Result<String> {
    if json {
        return render_json_or_text(record, "", true);
    }

    let mut lines = match record {
        MemoryRecord::Episode(episode) => vec![
            format!("Episode {}", episode.id),
            format!("layer: {}", episode.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!("confidence: {:.2}", episode.confidence),
            format!("content: {}", episode.content),
        ],
        MemoryRecord::Entity(entity) => vec![
            format!("Entity {}", entity.id),
            format!("layer: {}", entity.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!("name: {}", entity.canonical_name),
            format!(
                "aliases: {}",
                if entity.aliases.is_empty() {
                    "-".to_string()
                } else {
                    entity.aliases.join(", ")
                }
            ),
        ],
        MemoryRecord::Fact(fact) => vec![
            format!("Fact {}", fact.id),
            format!("layer: {}", fact.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!(
                "statement: {} {} {}",
                fact.subject_text, fact.predicate, fact.object_text
            ),
        ],
        MemoryRecord::Edge(edge) => vec![
            format!("Edge {}", edge.id),
            format!("layer: {}", edge.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!(
                "relation: {} {} {}",
                edge.subject_entity_id, edge.predicate, edge.object_entity_id
            ),
        ],
    };

    if let Some(archived_at) = memory_archived_at(record) {
        lines.push(format!("archived_at: {}", archived_at.to_rfc3339()));
    }
    if let Some(invalidated_at) = memory_invalidated_at(record) {
        lines.push(format!("invalidated_at: {}", invalidated_at.to_rfc3339()));
    }
    if let Some(valid_from) = memory_valid_from(record) {
        lines.push(format!("valid_from: {}", valid_from.to_rfc3339()));
    }
    if let Some(valid_to) = memory_valid_to(record) {
        lines.push(format!("valid_to: {}", valid_to.to_rfc3339()));
    }

    Ok(lines.join("\n"))
}

fn memory_summary(memory: &MemoryRecord) -> String {
    match memory {
        MemoryRecord::Episode(episode) => episode.content.clone(),
        MemoryRecord::Entity(entity) => entity.canonical_name.clone(),
        MemoryRecord::Fact(fact) => {
            format!(
                "{} {} {}",
                fact.subject_text, fact.predicate, fact.object_text
            )
        }
        MemoryRecord::Edge(edge) => {
            format!(
                "{} {} {}",
                edge.subject_entity_id, edge.predicate, edge.object_entity_id
            )
        }
    }
}

fn recall_reason_label(reason: &RecallReason) -> String {
    match reason {
        RecallReason::L0 => "l0".to_string(),
        RecallReason::L3 => "l3".to_string(),
        RecallReason::Exact => "exact".to_string(),
        RecallReason::Alias => "alias".to_string(),
        RecallReason::Bm25 => "bm25".to_string(),
        RecallReason::Vector => "vector".to_string(),
        RecallReason::Rerank => "rerank".to_string(),
        RecallReason::GraphHop { hops } => format!("graph_hop({hops})"),
        RecallReason::RecencyBoost => "recency_boost".to_string(),
        RecallReason::LayerBoost => "layer_boost".to_string(),
        RecallReason::HitFrequencyBoost => "hit_frequency_boost".to_string(),
        RecallReason::MmrSelected => "mmr_selected".to_string(),
    }
}

fn memory_status_label(record: &MemoryRecord) -> &'static str {
    if memory_invalidated_at(record).is_some() {
        "invalidated"
    } else if memory_archived_at(record).is_some() {
        "archived"
    } else {
        "active"
    }
}

fn memory_archived_at(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Episode(episode) => episode.archived_at,
        MemoryRecord::Entity(entity) => entity.archived_at,
        MemoryRecord::Fact(fact) => fact.archived_at,
        MemoryRecord::Edge(edge) => edge.archived_at,
    }
}

fn memory_invalidated_at(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Episode(episode) => episode.invalidated_at,
        MemoryRecord::Entity(entity) => entity.invalidated_at,
        MemoryRecord::Fact(fact) => fact.invalidated_at,
        MemoryRecord::Edge(edge) => edge.invalidated_at,
    }
}

fn memory_valid_from(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Fact(fact) => fact.valid_from,
        MemoryRecord::Edge(edge) => edge.valid_from,
        _ => None,
    }
}

fn memory_valid_to(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Fact(fact) => fact.valid_to,
        MemoryRecord::Edge(edge) => edge.valid_to,
        _ => None,
    }
}
