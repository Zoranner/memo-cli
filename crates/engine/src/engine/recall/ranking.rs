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
pub(super) fn dedupe_candidates_by_source(candidates: &mut Vec<Candidate>) {
    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.memory.source_key().to_string()));
}
pub(super) fn filter_candidates_by_query_coverage(query: &str, candidates: &mut Vec<Candidate>) {
    let query_tokens = lexical_tokens(query);
    if query_tokens.len() < 2 || candidates.len() < 2 {
        return;
    }
    let min_coverage = if query_tokens.len() <= 3 { 0.75 } else { 0.60 };

    let filtered = candidates
        .iter()
        .filter(|candidate| {
            let record_tokens = lexical_tokens(&candidate.memory.text_for_ranking());
            if record_tokens.is_empty() {
                return false;
            }
            let matched = query_tokens.intersection(&record_tokens).count();
            matched as f32 / query_tokens.len() as f32 >= min_coverage
        })
        .cloned()
        .collect::<Vec<_>>();

    if !filtered.is_empty() {
        *candidates = filtered;
    }
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
    let a_tokens = lexical_tokens(&a);
    let b_tokens = lexical_tokens(&b);
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
fn lexical_tokens(text: &str) -> HashSet<String> {
    normalize_text(text)
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(normalize_token)
        .collect()
}
fn normalize_token(token: &str) -> Option<String> {
    let token = token.trim();
    if token.len() < 3 || is_stopword(token) {
        return None;
    }

    let normalized = match token {
        "currently" => "current",
        "partnership" | "partners" => "partner",
        "builds" | "building" => "build",
        "makes" | "making" => "make",
        "ships" | "shipping" => "ship",
        "warehouses" => "warehouse",
        "drones" => "drone",
        "works" | "working" => "work",
        "succeeded" | "successfully" => "success",
        "failed" | "failure" => "fail",
        "cancelled" | "canceled" => "cancel",
        _ => token,
    };

    Some(normalized.to_string())
}
fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "about"
            | "after"
            | "and"
            | "are"
            | "based"
            | "does"
            | "for"
            | "from"
            | "has"
            | "into"
            | "is"
            | "the"
            | "this"
            | "what"
            | "where"
            | "which"
            | "who"
            | "with"
    )
}
