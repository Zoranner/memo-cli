use super::*;

pub(super) fn collect_graph_seeds<'a>(
    records: impl Iterator<Item = &'a MemoryRecord>,
) -> Vec<String> {
    let mut ids = HashSet::new();
    for record in records {
        match record {
            MemoryRecord::Entity(entity) => {
                ids.insert(entity.id.clone());
            }
            MemoryRecord::Fact(fact) => {
                if let Some(id) = &fact.subject_entity_id {
                    ids.insert(id.clone());
                }
                if let Some(id) = &fact.object_entity_id {
                    ids.insert(id.clone());
                }
            }
            _ => {}
        }
    }
    ids.into_iter().collect()
}
pub(super) fn add_candidate(target: &mut HashMap<String, Candidate>, candidate: Candidate) {
    let key = format!("{}:{}", candidate.memory.kind(), candidate.memory.id());
    target
        .entry(key)
        .and_modify(|existing| {
            existing.score = existing.score.max(candidate.score);
            existing.reasons.extend(candidate.reasons.iter().cloned());
        })
        .or_insert(candidate);
}
pub(super) fn recency_boost(updated_at: chrono::DateTime<chrono::Utc>) -> f32 {
    let age_days = (chrono::Utc::now() - updated_at).num_days().max(0) as f32;
    (-(age_days / 30.0)).exp() * 0.18
}
pub(super) fn hit_frequency_boost(hit_count: u64) -> f32 {
    ((hit_count as f32) + 1.0).ln() * 0.05
}
pub(super) fn mmr_select(mut candidates: Vec<Candidate>, limit: usize) -> Vec<Candidate> {
    if candidates.len() <= limit {
        return candidates;
    }

    let mut selected = Vec::new();
    while !candidates.is_empty() && selected.len() < limit {
        let (best_index, _) = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                let novelty_penalty = selected
                    .iter()
                    .map(|existing: &Candidate| {
                        text_similarity(
                            existing.memory.text_for_ranking(),
                            candidate.memory.text_for_ranking(),
                        )
                    })
                    .fold(0.0_f32, f32::max);
                let score = 0.7 * candidate.score - 0.3 * novelty_penalty;
                (index, score)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("candidate set is non-empty");
        selected.push(candidates.remove(best_index));
    }
    selected
}
fn text_similarity(a: String, b: String) -> f32 {
    let a_tokens: HashSet<_> = normalize_text(&a)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let b_tokens: HashSet<_> = normalize_text(&b)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }
    let intersection = a_tokens.intersection(&b_tokens).count() as f32;
    let union = a_tokens.union(&b_tokens).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}
