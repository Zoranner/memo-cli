use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    types::{
        EpisodeInput, FactInput, MemoryLayer, MemoryRecord, RecallRequest, RecallResult,
        RestoreScope,
    },
    DreamTrigger, EntityInput, MemoryEngine,
};

mod compare;
mod gate;

pub use compare::*;
pub use gate::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalDataset {
    pub name: String,
    #[serde(default)]
    pub memories: Vec<EvalMemory>,
    #[serde(default)]
    pub cases: Vec<EvalCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMemory {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub entities: Vec<EntityInput>,
    #[serde(default)]
    pub facts: Vec<FactInput>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub recorded_at: Option<DateTime<Utc>>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    pub id: String,
    #[serde(default = "default_eval_aspect")]
    pub aspect: String,
    pub query: String,
    #[serde(default)]
    pub expected_memory_ids: Vec<String>,
    #[serde(default)]
    pub forbidden_memory_ids: Vec<String>,
    #[serde(default = "default_eval_limit")]
    pub limit: usize,
    #[serde(default)]
    pub deep: bool,
    #[serde(default)]
    pub should_abstain: bool,
    #[serde(default)]
    pub dream_before_recall: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub dataset_name: String,
    pub case_count: usize,
    pub recall_at_1: f32,
    pub recall_at_5: f32,
    pub source_recall_at_1: f32,
    pub source_recall_at_5: f32,
    pub mrr: f32,
    pub source_mrr: f32,
    pub expected_hit_rate: f32,
    pub clean_hit_rate: f32,
    pub successful_case_rate: f32,
    pub precision_at_1: f32,
    pub precision_at_5: f32,
    pub clean_precision_at_5: f32,
    pub forbidden_rate: f32,
    pub noise_hit_rate: f32,
    pub mean_source_diversity: f32,
    pub mean_duplicate_rate: f32,
    pub abstention_correctness: f32,
    pub forbidden_correctness: f32,
    pub timing: EvalTimingReport,
    pub kind_counts: Vec<EvalKindCount>,
    pub failure_mode_counts: Vec<EvalFailureModeCount>,
    pub aspects: Vec<EvalAspectReport>,
    pub cases: Vec<EvalCaseReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCaseReport {
    pub id: String,
    pub aspect: String,
    pub query: String,
    pub result_count: usize,
    pub matched_rank: Option<usize>,
    pub matched_unique_rank: Option<usize>,
    pub has_expected_expectations: bool,
    pub has_forbidden_expectations: bool,
    pub forbidden_hit_count: usize,
    pub should_abstain: bool,
    pub expected_hit: bool,
    pub clean_hit: bool,
    pub successful: bool,
    pub abstention_correct: bool,
    pub dream_before_recall: bool,
    pub deep_search_used: bool,
    pub result_ids: Vec<String>,
    pub unique_result_ids: Vec<String>,
    pub unique_result_count: usize,
    pub duplicate_result_count: usize,
    pub duplicate_rate: f32,
    pub source_diversity: f32,
    pub timing_ms: f64,
    pub forbidden_result_ids: Vec<String>,
    pub failure_modes: Vec<String>,
    pub traces: Vec<EvalResultTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalAspectReport {
    pub aspect: String,
    pub case_count: usize,
    pub recall_at_1: f32,
    pub recall_at_5: f32,
    pub source_recall_at_1: f32,
    pub source_recall_at_5: f32,
    pub mrr: f32,
    pub source_mrr: f32,
    pub expected_hit_rate: f32,
    pub clean_hit_rate: f32,
    pub successful_case_rate: f32,
    pub precision_at_1: f32,
    pub precision_at_5: f32,
    pub clean_precision_at_5: f32,
    pub forbidden_rate: f32,
    pub noise_hit_rate: f32,
    pub mean_source_diversity: f32,
    pub mean_duplicate_rate: f32,
    pub abstention_correctness: f32,
    pub forbidden_correctness: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalTimingReport {
    pub total_ms: f64,
    pub load_memories_ms: f64,
    pub initial_restore_ms: f64,
    pub dream_ms: f64,
    pub recall_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResultTrace {
    pub rank: usize,
    pub unique_rank: Option<usize>,
    pub record_id: String,
    pub record_kind: String,
    pub source_memory_id: String,
    pub layer: String,
    pub score: f32,
    pub reasons: Vec<String>,
    pub expected: bool,
    pub forbidden: bool,
    pub duplicate: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalKindCount {
    pub kind: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalFailureModeCount {
    pub mode: String,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NormalizedPublicEvent {
    Memory {
        id: String,
        content: String,
        #[serde(default)]
        entities: Vec<EntityInput>,
        #[serde(default)]
        facts: Vec<FactInput>,
    },
    Query {
        id: String,
        #[serde(default = "default_eval_aspect")]
        aspect: String,
        query: String,
        #[serde(default)]
        expected_memory_ids: Vec<String>,
        #[serde(default)]
        forbidden_memory_ids: Vec<String>,
        #[serde(default = "default_eval_limit")]
        limit: usize,
        #[serde(default)]
        deep: bool,
        #[serde(default)]
        should_abstain: bool,
        #[serde(default)]
        dream_before_recall: bool,
    },
}

pub fn run_recall_eval(engine: &MemoryEngine, dataset: EvalDataset) -> Result<EvalReport> {
    let total_started = Instant::now();
    let load_started = Instant::now();
    let memory_ids = load_eval_memories(engine, &dataset.memories)?;
    let load_memories_ms = elapsed_ms(load_started);
    let restore_started = Instant::now();
    let _ = engine.restore_full(RestoreScope::Text)?;
    let initial_restore_ms = elapsed_ms(restore_started);
    let eval_ids_by_episode_id = memory_ids
        .iter()
        .map(|(eval_id, episode_id)| (episode_id.clone(), eval_id.clone()))
        .collect::<HashMap<_, _>>();
    let mut case_reports = Vec::with_capacity(dataset.cases.len());
    let mut dream_ms = 0.0;
    let mut recall_ms = 0.0;

    for case in &dataset.cases {
        if case.dream_before_recall {
            let dream_started = Instant::now();
            let _ = engine.dream(DreamTrigger::Manual)?;
            let _ = engine.restore_full(RestoreScope::Text)?;
            dream_ms += elapsed_ms(dream_started);
        }
        validate_case_memory_ids(case, &memory_ids)?;
        let expected_episode_ids = case
            .expected_memory_ids
            .iter()
            .filter_map(|id| memory_ids.get(id))
            .cloned()
            .collect::<Vec<_>>();
        let has_expected_expectations = !expected_episode_ids.is_empty();
        let forbidden_episode_ids = case
            .forbidden_memory_ids
            .iter()
            .filter_map(|id| memory_ids.get(id))
            .cloned()
            .collect::<Vec<_>>();
        let has_forbidden_expectations = !forbidden_episode_ids.is_empty();
        let recall_started = Instant::now();
        let result_set = engine.recall(RecallRequest {
            query: case.query.clone(),
            limit: case.limit,
            deep: case.deep,
        });
        let result_set = result_set?;
        let case_timing_ms = elapsed_ms(recall_started);
        recall_ms += case_timing_ms;
        let matched_rank = first_matching_rank(&result_set.results, &expected_episode_ids);
        let forbidden_result_ids = forbidden_result_ids(
            &result_set.results,
            &forbidden_episode_ids,
            &eval_ids_by_episode_id,
        );
        let expected_hit = matched_rank.is_some();
        let clean_hit =
            has_expected_expectations && expected_hit && forbidden_result_ids.is_empty();
        let abstention_correct = case.should_abstain && result_set.results.is_empty();
        let successful = if case.should_abstain {
            abstention_correct
        } else if has_expected_expectations {
            clean_hit
        } else {
            result_set.results.is_empty()
        };
        let result_ids = result_set
            .results
            .iter()
            .map(|result| eval_memory_id_for_record(&result.memory, &eval_ids_by_episode_id))
            .collect::<Vec<_>>();
        let unique_result_ids = unique_result_ids(&result_ids);
        let unique_result_count = unique_result_ids.len();
        let duplicate_result_count = result_ids.len().saturating_sub(unique_result_ids.len());
        let duplicate_rate = if result_ids.is_empty() {
            0.0
        } else {
            duplicate_result_count as f32 / result_ids.len() as f32
        };
        let source_diversity = if result_ids.is_empty() {
            1.0
        } else {
            unique_result_count as f32 / result_ids.len() as f32
        };
        let matched_unique_rank =
            first_matching_eval_rank(&unique_result_ids, &case.expected_memory_ids);
        let traces = build_result_traces(
            &result_set.results,
            &case.expected_memory_ids,
            &case.forbidden_memory_ids,
            &eval_ids_by_episode_id,
        );
        let failure_modes = failure_modes(
            case,
            has_expected_expectations,
            expected_hit,
            abstention_correct,
            forbidden_result_ids.is_empty(),
        );
        case_reports.push(EvalCaseReport {
            id: case.id.clone(),
            aspect: case.aspect.clone(),
            query: case.query.clone(),
            result_count: result_set.results.len(),
            matched_rank,
            matched_unique_rank,
            has_expected_expectations,
            has_forbidden_expectations,
            forbidden_hit_count: forbidden_result_ids.len(),
            should_abstain: case.should_abstain,
            expected_hit,
            clean_hit,
            successful,
            abstention_correct,
            dream_before_recall: case.dream_before_recall,
            deep_search_used: result_set.deep_search_used,
            result_ids,
            unique_result_ids,
            unique_result_count,
            duplicate_result_count,
            duplicate_rate,
            source_diversity,
            timing_ms: case_timing_ms,
            forbidden_result_ids,
            failure_modes,
            traces,
        });
    }

    let mut report = build_report(dataset.name, case_reports);
    report.timing = EvalTimingReport {
        total_ms: elapsed_ms(total_started),
        load_memories_ms,
        initial_restore_ms,
        dream_ms,
        recall_ms,
    };
    Ok(report)
}

pub fn dataset_from_normalized_public_jsonl(name: &str, raw: &str) -> Result<EvalDataset> {
    let mut memories = Vec::new();
    let mut cases = Vec::new();
    for (line_index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<NormalizedPublicEvent>(trimmed)
            .with_context(|| format!("failed to parse public eval jsonl line {}", line_index + 1))?
        {
            NormalizedPublicEvent::Memory {
                id,
                content,
                entities,
                facts,
            } => memories.push(EvalMemory {
                id,
                content,
                entities,
                facts,
                session_id: None,
                recorded_at: None,
                confidence: default_confidence(),
            }),
            NormalizedPublicEvent::Query {
                id,
                aspect,
                query,
                expected_memory_ids,
                forbidden_memory_ids,
                limit,
                deep,
                should_abstain,
                dream_before_recall,
            } => {
                if !should_abstain && expected_memory_ids.is_empty() {
                    bail!(
                        "public eval query {} must provide expected_memory_ids unless should_abstain is true",
                        id
                    );
                }
                cases.push(EvalCase {
                    id,
                    aspect,
                    query,
                    expected_memory_ids,
                    forbidden_memory_ids,
                    limit,
                    deep,
                    should_abstain,
                    dream_before_recall,
                });
            }
        }
    }
    Ok(EvalDataset {
        name: name.to_string(),
        memories,
        cases,
    })
}

fn load_eval_memories(
    engine: &MemoryEngine,
    memories: &[EvalMemory],
) -> Result<HashMap<String, String>> {
    let mut ids = HashMap::new();
    for memory in memories {
        let episode_id = engine.remember(EpisodeInput {
            content: memory.content.clone(),
            layer: MemoryLayer::L1,
            entities: memory.entities.clone(),
            facts: memory.facts.clone(),
            source_episode_id: None,
            session_id: memory.session_id.clone(),
            recorded_at: memory.recorded_at,
            confidence: memory.confidence,
        })?;
        ids.insert(memory.id.clone(), episode_id);
    }
    Ok(ids)
}

fn validate_case_memory_ids(case: &EvalCase, memory_ids: &HashMap<String, String>) -> Result<()> {
    if !case.should_abstain && case.expected_memory_ids.is_empty() {
        bail!(
            "eval case {} must set expected_memory_ids unless should_abstain is true",
            case.id
        );
    }
    if case.should_abstain && !case.expected_memory_ids.is_empty() {
        bail!(
            "eval case {} cannot set expected_memory_ids when should_abstain is true",
            case.id
        );
    }

    let mut missing_ids = Vec::new();
    for id in &case.expected_memory_ids {
        if !memory_ids.contains_key(id) {
            missing_ids.push(format!("expected:{id}"));
        }
    }
    for id in &case.forbidden_memory_ids {
        if !memory_ids.contains_key(id) {
            missing_ids.push(format!("forbidden:{id}"));
        }
    }
    if !missing_ids.is_empty() {
        bail!(
            "eval case {} references unknown memory ids: {}",
            case.id,
            missing_ids.join(", ")
        );
    }
    Ok(())
}

fn first_matching_rank(results: &[RecallResult], expected_episode_ids: &[String]) -> Option<usize> {
    if expected_episode_ids.is_empty() {
        return None;
    }
    results
        .iter()
        .position(|result| record_matches_expected(&result.memory, expected_episode_ids))
        .map(|index| index + 1)
}

fn first_matching_eval_rank(
    result_ids: &[String],
    expected_memory_ids: &[String],
) -> Option<usize> {
    if expected_memory_ids.is_empty() {
        return None;
    }
    result_ids
        .iter()
        .position(|result_id| {
            expected_memory_ids
                .iter()
                .any(|expected| expected == result_id)
        })
        .map(|index| index + 1)
}

fn failure_modes(
    case: &EvalCase,
    has_expected_expectations: bool,
    expected_hit: bool,
    abstention_correct: bool,
    no_forbidden_hits: bool,
) -> Vec<String> {
    let mut modes = Vec::new();
    if case.should_abstain && !abstention_correct {
        modes.push("non_abstained".to_string());
    }
    if has_expected_expectations && !expected_hit {
        modes.push("missed_expected".to_string());
    }
    if has_expected_expectations && expected_hit && !no_forbidden_hits {
        modes.push("forbidden_hit".to_string());
    }
    if !case.should_abstain && !has_expected_expectations {
        modes.push("missing_expectation".to_string());
    }
    modes
}

fn forbidden_result_ids(
    results: &[RecallResult],
    forbidden_episode_ids: &[String],
    eval_ids_by_episode_id: &HashMap<String, String>,
) -> Vec<String> {
    if forbidden_episode_ids.is_empty() {
        return Vec::new();
    }
    results
        .iter()
        .filter(|result| record_matches_expected(&result.memory, forbidden_episode_ids))
        .map(|result| eval_memory_id_for_record(&result.memory, eval_ids_by_episode_id))
        .collect()
}

fn eval_memory_id_for_record(
    record: &MemoryRecord,
    eval_ids_by_episode_id: &HashMap<String, String>,
) -> String {
    if let Some(eval_id) = eval_ids_by_episode_id.get(record.id()) {
        return eval_id.clone();
    }
    if let Some(source_episode_id) = record.source_episode_id() {
        if let Some(eval_id) = eval_ids_by_episode_id.get(source_episode_id) {
            return eval_id.clone();
        }
    }
    record.id().to_string()
}

fn unique_result_ids(result_ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for id in result_ids {
        if seen.insert(id.clone()) {
            unique.push(id.clone());
        }
    }
    unique
}

fn build_result_traces(
    results: &[RecallResult],
    expected_memory_ids: &[String],
    forbidden_memory_ids: &[String],
    eval_ids_by_episode_id: &HashMap<String, String>,
) -> Vec<EvalResultTrace> {
    let result_ids = results
        .iter()
        .map(|result| eval_memory_id_for_record(&result.memory, eval_ids_by_episode_id))
        .collect::<Vec<_>>();
    let unique_ids = unique_result_ids(&result_ids);
    let unique_ranks = unique_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index + 1))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();

    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let source_memory_id =
                eval_memory_id_for_record(&result.memory, eval_ids_by_episode_id);
            let duplicate = !seen.insert(source_memory_id.clone());
            EvalResultTrace {
                rank: index + 1,
                unique_rank: unique_ranks.get(&source_memory_id).copied(),
                record_id: result.memory.id().to_string(),
                record_kind: result.memory.kind().to_string(),
                source_memory_id: source_memory_id.clone(),
                layer: result.memory.layer().as_str().to_string(),
                score: result.score,
                reasons: result
                    .reasons
                    .iter()
                    .map(|reason| format!("{reason:?}"))
                    .collect(),
                expected: expected_memory_ids
                    .iter()
                    .any(|expected| expected == &source_memory_id),
                forbidden: forbidden_memory_ids
                    .iter()
                    .any(|forbidden| forbidden == &source_memory_id),
                duplicate,
                active: result.memory.is_active(),
            }
        })
        .collect()
}

fn record_matches_expected(record: &MemoryRecord, expected_episode_ids: &[String]) -> bool {
    if expected_episode_ids.iter().any(|id| id == record.id()) {
        return true;
    }
    record.source_episode_id().is_some_and(|source_episode_id| {
        expected_episode_ids
            .iter()
            .any(|id| id == source_episode_id)
    })
}

fn build_report(dataset_name: String, cases: Vec<EvalCaseReport>) -> EvalReport {
    let summary = summarize_cases(&cases);
    let mut grouped = HashMap::<String, Vec<EvalCaseReport>>::new();
    for case in cases.iter().cloned() {
        grouped.entry(case.aspect.clone()).or_default().push(case);
    }
    let mut aspects = grouped
        .into_iter()
        .map(|(aspect, cases)| {
            let summary = summarize_cases(&cases);
            EvalAspectReport {
                aspect,
                case_count: summary.case_count,
                recall_at_1: summary.recall_at_1,
                recall_at_5: summary.recall_at_5,
                source_recall_at_1: summary.source_recall_at_1,
                source_recall_at_5: summary.source_recall_at_5,
                mrr: summary.mrr,
                source_mrr: summary.source_mrr,
                expected_hit_rate: summary.expected_hit_rate,
                clean_hit_rate: summary.clean_hit_rate,
                successful_case_rate: summary.successful_case_rate,
                precision_at_1: summary.precision_at_1,
                precision_at_5: summary.precision_at_5,
                clean_precision_at_5: summary.clean_precision_at_5,
                forbidden_rate: summary.forbidden_rate,
                noise_hit_rate: summary.noise_hit_rate,
                mean_source_diversity: summary.mean_source_diversity,
                mean_duplicate_rate: summary.mean_duplicate_rate,
                abstention_correctness: summary.abstention_correctness,
                forbidden_correctness: summary.forbidden_correctness,
            }
        })
        .collect::<Vec<_>>();
    aspects.sort_by(|left, right| left.aspect.cmp(&right.aspect));

    EvalReport {
        dataset_name,
        case_count: summary.case_count,
        recall_at_1: summary.recall_at_1,
        recall_at_5: summary.recall_at_5,
        source_recall_at_1: summary.source_recall_at_1,
        source_recall_at_5: summary.source_recall_at_5,
        mrr: summary.mrr,
        source_mrr: summary.source_mrr,
        expected_hit_rate: summary.expected_hit_rate,
        clean_hit_rate: summary.clean_hit_rate,
        successful_case_rate: summary.successful_case_rate,
        precision_at_1: summary.precision_at_1,
        precision_at_5: summary.precision_at_5,
        clean_precision_at_5: summary.clean_precision_at_5,
        forbidden_rate: summary.forbidden_rate,
        noise_hit_rate: summary.noise_hit_rate,
        mean_source_diversity: summary.mean_source_diversity,
        mean_duplicate_rate: summary.mean_duplicate_rate,
        abstention_correctness: summary.abstention_correctness,
        forbidden_correctness: summary.forbidden_correctness,
        timing: EvalTimingReport::default(),
        kind_counts: kind_counts(&cases),
        failure_mode_counts: failure_mode_counts(&cases),
        aspects,
        cases,
    }
}

struct EvalSummary {
    case_count: usize,
    recall_at_1: f32,
    recall_at_5: f32,
    source_recall_at_1: f32,
    source_recall_at_5: f32,
    mrr: f32,
    source_mrr: f32,
    expected_hit_rate: f32,
    clean_hit_rate: f32,
    successful_case_rate: f32,
    precision_at_1: f32,
    precision_at_5: f32,
    clean_precision_at_5: f32,
    forbidden_rate: f32,
    noise_hit_rate: f32,
    mean_source_diversity: f32,
    mean_duplicate_rate: f32,
    abstention_correctness: f32,
    forbidden_correctness: f32,
}

fn summarize_cases(cases: &[EvalCaseReport]) -> EvalSummary {
    let case_count = cases.len();
    let denominator = case_count.max(1) as f32;
    let recall_at_1 = cases
        .iter()
        .filter(|case| case.matched_rank.is_some_and(|rank| rank <= 1))
        .count() as f32
        / denominator;
    let recall_at_5 = cases
        .iter()
        .filter(|case| case.matched_rank.is_some_and(|rank| rank <= 5))
        .count() as f32
        / denominator;
    let source_recall_at_1 = cases
        .iter()
        .filter(|case| case.matched_unique_rank.is_some_and(|rank| rank <= 1))
        .count() as f32
        / denominator;
    let source_recall_at_5 = cases
        .iter()
        .filter(|case| case.matched_unique_rank.is_some_and(|rank| rank <= 5))
        .count() as f32
        / denominator;
    let expected_hit_rate = cases
        .iter()
        .filter(|case| case.matched_rank.is_some())
        .count() as f32
        / denominator;
    let expected_total = cases
        .iter()
        .filter(|case| case.has_expected_expectations)
        .count();
    let clean_hit_rate = if expected_total == 0 {
        1.0
    } else {
        cases.iter().filter(|case| case.clean_hit).count() as f32 / expected_total as f32
    };
    let successful_case_rate =
        cases.iter().filter(|case| case.successful).count() as f32 / denominator;
    let precision_at_1 = cases.iter().map(|case| precision_at(case, 1)).sum::<f32>() / denominator;
    let precision_at_5 = cases.iter().map(|case| precision_at(case, 5)).sum::<f32>() / denominator;
    let clean_precision_at_5 = cases
        .iter()
        .map(|case| clean_precision_at(case, 5))
        .sum::<f32>()
        / denominator;
    let total_traces = cases
        .iter()
        .map(|case| case.traces.len())
        .sum::<usize>()
        .max(1) as f32;
    let forbidden_rate = cases
        .iter()
        .flat_map(|case| case.traces.iter())
        .filter(|trace| trace.forbidden)
        .count() as f32
        / total_traces;
    let noise_hit_rate = cases
        .iter()
        .flat_map(|case| case.traces.iter())
        .filter(|trace| !trace.expected && !trace.forbidden)
        .count() as f32
        / total_traces;
    let mean_source_diversity =
        cases.iter().map(|case| case.source_diversity).sum::<f32>() / denominator;
    let mean_duplicate_rate =
        cases.iter().map(|case| case.duplicate_rate).sum::<f32>() / denominator;
    let abstention_total = cases.iter().filter(|case| case.should_abstain).count();
    let abstention_correctness = if abstention_total == 0 {
        1.0
    } else {
        cases.iter().filter(|case| case.abstention_correct).count() as f32 / abstention_total as f32
    };
    let forbidden_total = cases
        .iter()
        .filter(|case| case.has_forbidden_expectations)
        .count();
    let forbidden_cases = cases
        .iter()
        .filter(|case| !case.forbidden_result_ids.is_empty())
        .count();
    let forbidden_correctness = if forbidden_total == 0 {
        1.0
    } else {
        (forbidden_total - forbidden_cases) as f32 / forbidden_total as f32
    };
    let mrr = cases
        .iter()
        .map(|case| case.matched_rank.map_or(0.0, |rank| 1.0 / rank as f32))
        .sum::<f32>()
        / denominator;
    let source_mrr = cases
        .iter()
        .map(|case| {
            case.matched_unique_rank
                .map_or(0.0, |rank| 1.0 / rank as f32)
        })
        .sum::<f32>()
        / denominator;

    EvalSummary {
        case_count,
        recall_at_1,
        recall_at_5,
        source_recall_at_1,
        source_recall_at_5,
        mrr,
        source_mrr,
        expected_hit_rate,
        clean_hit_rate,
        successful_case_rate,
        precision_at_1,
        precision_at_5,
        clean_precision_at_5,
        forbidden_rate,
        noise_hit_rate,
        mean_source_diversity,
        mean_duplicate_rate,
        abstention_correctness,
        forbidden_correctness,
    }
}

fn default_eval_limit() -> usize {
    5
}

fn default_eval_aspect() -> String {
    "general".to_string()
}

fn default_confidence() -> f32 {
    0.85
}

fn precision_at(case: &EvalCaseReport, k: usize) -> f32 {
    let denominator = case.traces.len().min(k);
    if denominator == 0 {
        return 0.0;
    }
    case.traces
        .iter()
        .take(k)
        .filter(|trace| trace.expected)
        .count() as f32
        / denominator as f32
}

fn clean_precision_at(case: &EvalCaseReport, k: usize) -> f32 {
    let denominator = case.traces.len().min(k);
    if denominator == 0 {
        return 0.0;
    }
    let expected_hits = case
        .traces
        .iter()
        .take(k)
        .filter(|trace| trace.expected)
        .count();
    let forbidden_hits = case
        .traces
        .iter()
        .take(k)
        .filter(|trace| trace.forbidden)
        .count();
    expected_hits.saturating_sub(forbidden_hits) as f32 / denominator as f32
}

fn kind_counts(cases: &[EvalCaseReport]) -> Vec<EvalKindCount> {
    let mut counts = HashMap::<String, usize>::new();
    for trace in cases.iter().flat_map(|case| case.traces.iter()) {
        *counts.entry(trace.record_kind.clone()).or_default() += 1;
    }
    let mut counts = counts
        .into_iter()
        .map(|(kind, count)| EvalKindCount { kind, count })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| left.kind.cmp(&right.kind));
    counts
}

fn failure_mode_counts(cases: &[EvalCaseReport]) -> Vec<EvalFailureModeCount> {
    let mut counts = HashMap::<String, usize>::new();
    for mode in cases.iter().flat_map(|case| case.failure_modes.iter()) {
        *counts.entry(mode.clone()).or_default() += 1;
    }
    let mut counts = counts
        .into_iter()
        .map(|(mode, count)| EvalFailureModeCount { mode, count })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| left.mode.cmp(&right.mode));
    counts
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

trait EvalMemoryRecordExt {
    fn source_episode_id(&self) -> Option<&str>;
}

impl EvalMemoryRecordExt for MemoryRecord {
    fn source_episode_id(&self) -> Option<&str> {
        match self {
            MemoryRecord::Episode(_) => None,
            MemoryRecord::Entity(record) => record.source_episode_id.as_deref(),
            MemoryRecord::Fact(record) => record.source_episode_id.as_deref(),
            MemoryRecord::Edge(record) => record.source_episode_id.as_deref(),
        }
    }
}
