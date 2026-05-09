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
pub(super) fn dedupe_candidates_by_source(
    candidates: &mut Vec<Candidate>,
    keep_graph_facts: bool,
    keep_graph_edges: bool,
) {
    let mut by_source = HashMap::<String, Candidate>::new();
    for candidate in candidates.drain(..) {
        let key = candidate_source_dedupe_key(&candidate, keep_graph_facts, keep_graph_edges);
        by_source
            .entry(key)
            .and_modify(|existing| {
                if compare_source_dedupe_candidate(&candidate, existing).is_gt() {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    *candidates = by_source.into_values().collect();
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
}

fn candidate_source_dedupe_key(
    candidate: &Candidate,
    keep_graph_facts: bool,
    keep_graph_edges: bool,
) -> String {
    if candidate
        .reasons
        .iter()
        .any(|reason| matches!(reason, RecallReason::GraphHop { .. }))
    {
        match candidate.memory {
            MemoryRecord::Fact(_) if keep_graph_facts => {
                return format!("graph-fact:{}", candidate.memory.source_key());
            }
            MemoryRecord::Edge(_) if keep_graph_edges => {
                return format!("graph-edge:{}", candidate.memory.source_key());
            }
            _ => {}
        }
    }

    candidate.memory.source_key().to_string()
}

fn candidate_kind_priority(memory: &MemoryRecord) -> u8 {
    match memory {
        MemoryRecord::Fact(_) => 3,
        MemoryRecord::Entity(_) => 2,
        MemoryRecord::Episode(_) => 1,
        MemoryRecord::Edge(_) => 0,
    }
}

fn compare_source_dedupe_candidate(left: &Candidate, right: &Candidate) -> std::cmp::Ordering {
    candidate_kind_priority(&left.memory)
        .cmp(&candidate_kind_priority(&right.memory))
        .then(left.score.total_cmp(&right.score))
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
pub(super) fn answer_shape_boost(query: &str, memory: &MemoryRecord) -> f32 {
    if !looks_like_current_location_query(query) {
        return 0.0;
    }

    match memory {
        MemoryRecord::Fact(fact) if fact.predicate == "lives_in" => 0.35,
        MemoryRecord::Episode(episode) => {
            let tokens = lexical_tokens(&episode.content);
            if tokens.contains("live") || tokens.contains("move") || tokens.contains("home") {
                0.20
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}
pub(super) fn subject_coverage_boost(query: &str, memory: &MemoryRecord) -> f32 {
    let subject_tokens = subject_tokens(query);
    if subject_tokens.is_empty() {
        return 0.0;
    }

    let memory_tokens = lexical_tokens(&memory.text_for_ranking());
    let matched = subject_tokens.intersection(&memory_tokens).count();
    if matched > 0 {
        0.35 * matched as f32
    } else {
        -0.18
    }
}
pub(super) fn query_subject_tokens(query: &str) -> HashSet<String> {
    subject_tokens(query)
}
pub(super) fn memory_contains_subject(memory: &MemoryRecord, subject: &str) -> bool {
    lexical_tokens(&memory.text_for_ranking()).contains(subject)
}
pub(super) fn query_coverage(query: &str, memory: &MemoryRecord) -> f32 {
    let query_tokens = lexical_tokens(query);
    if query_tokens.is_empty() {
        return 0.0;
    }
    let record_tokens = lexical_tokens(&memory.text_for_ranking());
    if record_tokens.is_empty() {
        return 0.0;
    }
    query_tokens.intersection(&record_tokens).count() as f32 / query_tokens.len() as f32
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
        "recently" => "recent",
        "lives" | "lived" | "living" => "live",
        "moved" | "moving" => "move",
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
fn looks_like_current_location_query(query: &str) -> bool {
    let tokens = lexical_tokens(query);
    tokens.contains("where")
        || tokens.contains("current")
        || tokens.contains("recent")
        || tokens.contains("live")
        || tokens.contains("home")
        || tokens.contains("move")
}
fn subject_tokens(query: &str) -> HashSet<String> {
    query
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.chars().next().is_some_and(char::is_uppercase))
        .filter_map(|token| normalize_token(&token.to_ascii_lowercase()))
        .filter(|token| !is_query_modifier(token))
        .collect()
}
fn is_query_modifier(token: &str) -> bool {
    matches!(
        token,
        "current"
            | "recent"
            | "live"
            | "move"
            | "home"
            | "city"
            | "office"
            | "location"
            | "role"
            | "title"
            | "job"
            | "position"
            | "base"
            | "transfer"
            | "promote"
    )
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
