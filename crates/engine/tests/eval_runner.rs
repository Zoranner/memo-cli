use anyhow::Result;
use chrono::{TimeZone, Utc};
use memo_engine::{
    eval::{
        compare_eval_reports, dataset_from_normalized_public_jsonl, evaluate_recall_quality_gate,
        run_recall_eval, EvalCase, EvalCompareOptions, EvalDataset, EvalMemory, EvalReport,
        EvalTimingReport, RecallQualityGateProfile,
    },
    EngineConfig, EntityInput, ExtractionSource, FactInput, MemoryEngine,
};
use tempfile::TempDir;

#[test]
fn recall_eval_reports_core_retrieval_metrics() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()))?;
    let dataset = EvalDataset {
        name: "basic synthetic".to_string(),
        memories: vec![
            EvalMemory {
                id: "alice-profile".to_string(),
                content: "Alice is also known as Ally.".to_string(),
                entities: vec![EntityInput {
                    entity_type: "person".to_string(),
                    name: "Alice".to_string(),
                    aliases: vec!["Ally".to_string()],
                    confidence: 0.9,
                    source: ExtractionSource::Manual,
                }],
                facts: Vec::new(),
                session_id: None,
                recorded_at: None,
                confidence: 0.85,
            },
            EvalMemory {
                id: "alice-paris".to_string(),
                content: "Alice lives in Paris.".to_string(),
                entities: Vec::new(),
                facts: vec![FactInput {
                    subject: "Alice".to_string(),
                    predicate: "lives_in".to_string(),
                    object: "Paris".to_string(),
                    confidence: 0.9,
                    source: ExtractionSource::Manual,
                }],
                session_id: None,
                recorded_at: None,
                confidence: 0.85,
            },
        ],
        cases: vec![
            EvalCase {
                id: "alias".to_string(),
                aspect: "alias".to_string(),
                query: "Ally".to_string(),
                expected_memory_ids: vec!["alice-profile".to_string()],
                forbidden_memory_ids: Vec::new(),
                limit: 5,
                deep: false,
                should_abstain: false,
                dream_before_recall: false,
            },
            EvalCase {
                id: "unknown".to_string(),
                aspect: "abstention".to_string(),
                query: "Bob deployment target".to_string(),
                expected_memory_ids: Vec::new(),
                forbidden_memory_ids: Vec::new(),
                limit: 5,
                deep: false,
                should_abstain: true,
                dream_before_recall: false,
            },
        ],
    };

    let report = run_recall_eval(&engine, dataset)?;

    assert_eq!(report.dataset_name, "basic synthetic");
    assert_eq!(report.case_count, 2);
    assert_eq!(report.recall_at_1, 1.0);
    assert_eq!(report.recall_at_5, 1.0);
    assert_eq!(report.source_recall_at_1, 1.0);
    assert_eq!(report.source_recall_at_5, 1.0);
    assert_eq!(report.expected_hit_rate, 1.0);
    assert_eq!(report.clean_hit_rate, 1.0);
    assert_eq!(report.successful_case_rate, 1.0);
    assert_eq!(report.precision_at_1, 0.5);
    assert_eq!(report.precision_at_5, 0.25);
    assert_eq!(report.clean_precision_at_5, 0.25);
    assert_eq!(report.forbidden_rate, 0.0);
    assert!(report.noise_hit_rate > 0.0);
    assert_eq!(report.abstention_correctness, 1.0);
    assert!(report.mean_source_diversity > 0.0);
    assert!(report.mean_source_diversity <= 1.0);
    assert!(report.mean_duplicate_rate >= 0.0);
    assert!(report.mean_duplicate_rate <= 1.0);
    assert!(report.mrr >= 0.5);
    assert!(report.source_mrr >= 0.5);
    assert!(report.timing.total_ms >= report.timing.load_memories_ms);
    assert!(
        report
            .kind_counts
            .iter()
            .map(|count| count.count)
            .sum::<usize>()
            > 0
    );
    assert!(report
        .kind_counts
        .iter()
        .any(|count| count.kind == "entity"));
    assert!(report.aspects.iter().any(|aspect| aspect.aspect == "alias"
        && aspect.case_count == 1
        && aspect.recall_at_1 == 1.0));
    assert!(report
        .aspects
        .iter()
        .any(|aspect| aspect.aspect == "abstention"
            && aspect.case_count == 1
            && aspect.abstention_correctness == 1.0));
    assert!(report.cases.iter().any(|case| case.id == "alias"
        && case.aspect == "alias"
        && case.matched_rank == Some(1)
        && case.matched_unique_rank == Some(1)
        && case.clean_hit
        && case.source_diversity > 0.0
        && case.source_diversity <= 1.0
        && case.duplicate_rate >= 0.0
        && case.duplicate_rate <= 1.0
        && case.failure_modes.is_empty()
        && !case.traces.is_empty()
        && case.traces[0].rank == 1
        && case.traces[0].record_kind == "entity"
        && case.traces[0].source_memory_id == "alice-profile"
        && case.traces[0].expected
        && case.traces[0]
            .reasons
            .iter()
            .any(|reason| reason == "Alias")
        && case.result_ids.contains(&"alice-profile".to_string())));
    let alias_case = report
        .cases
        .iter()
        .find(|case| case.id == "alias")
        .expect("alias case report");
    assert_eq!(
        alias_case
            .unique_result_ids
            .iter()
            .filter(|id| id.as_str() == "alice-profile")
            .count(),
        1
    );
    assert!(alias_case.duplicate_result_count <= alias_case.result_count);
    assert!(alias_case.unique_result_count <= alias_case.result_count);
    assert!(alias_case.timing_ms > 0.0);
    assert!(report.cases.iter().any(|case| case.id == "unknown"
        && case.should_abstain
        && case.abstention_correct
        && case.successful
        && case.failure_modes.is_empty()));
    Ok(())
}

#[test]
fn eval_compare_reports_quality_regressions() -> Result<()> {
    let mut baseline = empty_report("baseline");
    baseline.recall_at_1 = 0.9;
    baseline.clean_hit_rate = 0.8;
    baseline.forbidden_rate = 0.1;
    baseline.mean_duplicate_rate = 0.2;
    baseline.timing.total_ms = 100.0;

    let mut current = baseline.clone();
    current.recall_at_1 = 0.8;
    current.clean_hit_rate = 0.6;
    current.forbidden_rate = 0.3;
    current.mean_duplicate_rate = 0.5;
    current.timing.total_ms = 150.0;

    let comparison = compare_eval_reports(&baseline, &current, EvalCompareOptions::default());

    assert!(!comparison.passed);
    assert!(comparison
        .regressions
        .iter()
        .any(|regression| regression.metric == "recall_at_1"));
    assert!(comparison
        .regressions
        .iter()
        .any(|regression| regression.metric == "forbidden_rate"));
    Ok(())
}

#[test]
fn recall_quality_gate_separates_failures_from_known_risks() -> Result<()> {
    let mut report = empty_report("quality");
    report.recall_at_1 = 0.50;
    report.recall_at_5 = 0.90;
    report.mrr = 0.72;
    report.source_mrr = 0.73;
    report.clean_hit_rate = 0.55;
    report.successful_case_rate = 0.60;
    report.abstention_correctness = 1.0;
    report.forbidden_correctness = 0.0;
    report.mean_duplicate_rate = 0.31;

    let gate = evaluate_recall_quality_gate(&report, RecallQualityGateProfile::synthetic_quality());

    assert!(!gate.passed);
    assert!(gate
        .failures
        .iter()
        .any(|failure| failure.metric == "recall_at_1"
            && (failure.expected - 0.60).abs() < f32::EPSILON
            && (failure.actual - 0.50).abs() < f32::EPSILON));
    assert!(gate
        .failures
        .iter()
        .any(|failure| failure.metric == "successful_case_rate"));
    assert!(gate
        .failures
        .iter()
        .any(|failure| failure.metric == "forbidden_correctness"));
    assert!(gate
        .failures
        .iter()
        .any(|failure| failure.metric == "mean_duplicate_rate"));
    Ok(())
}

#[test]
fn normalized_public_jsonl_converts_to_eval_dataset() -> Result<()> {
    let raw = r#"
{"type":"memory","id":"m1","content":"Alice confirmed London is her current home.","facts":[{"subject":"Alice","predicate":"lives_in","object":"London"}]}
{"type":"query","id":"q1","aspect":"public_temporal","query":"Where does Alice currently live?","expected_memory_ids":["m1"],"deep":true}
{"type":"query","id":"q2","aspect":"public_abstention","query":"Where does Bob live?","should_abstain":true}
"#;

    let dataset = dataset_from_normalized_public_jsonl("public normalized", raw)?;

    assert_eq!(dataset.name, "public normalized");
    assert_eq!(dataset.memories.len(), 1);
    assert_eq!(dataset.cases.len(), 2);
    assert_eq!(dataset.cases[0].expected_memory_ids, vec!["m1"]);
    assert!(dataset.cases[1].should_abstain);
    Ok(())
}

fn empty_report(name: &str) -> EvalReport {
    EvalReport {
        dataset_name: name.to_string(),
        case_count: 0,
        recall_at_1: 0.0,
        recall_at_5: 0.0,
        source_recall_at_1: 0.0,
        source_recall_at_5: 0.0,
        mrr: 0.0,
        source_mrr: 0.0,
        expected_hit_rate: 0.0,
        clean_hit_rate: 0.0,
        successful_case_rate: 0.0,
        precision_at_1: 0.0,
        precision_at_5: 0.0,
        clean_precision_at_5: 0.0,
        forbidden_rate: 0.0,
        noise_hit_rate: 0.0,
        mean_source_diversity: 0.0,
        mean_duplicate_rate: 0.0,
        abstention_correctness: 0.0,
        forbidden_correctness: 0.0,
        timing: EvalTimingReport::default(),
        kind_counts: Vec::new(),
        failure_mode_counts: Vec::new(),
        aspects: Vec::new(),
        cases: Vec::new(),
    }
}

#[test]
fn synthetic_dataset_files_are_valid() -> Result<()> {
    let datasets = [
        include_str!("../../../evals/synthetic/basic.json"),
        include_str!("../../../evals/synthetic/smoke.json"),
        include_str!("../../../evals/synthetic/quality.json"),
        include_str!("../../../evals/synthetic/stress.json"),
        include_str!("../../../evals/synthetic/temporal.json"),
        include_str!("../../../evals/synthetic/adversarial.json"),
    ];
    let parsed = datasets
        .iter()
        .map(|raw| serde_json::from_str::<EvalDataset>(raw))
        .collect::<Result<Vec<_>, _>>()?;

    let basic = parsed
        .iter()
        .find(|d| d.name == "basic synthetic recall")
        .expect("basic dataset");
    assert_eq!(basic.memories.len(), 8);
    assert_eq!(basic.cases.len(), 9);
    assert!(basic
        .cases
        .iter()
        .any(|case| case.id == "alias-alice" && case.aspect == "alias"));
    assert!(basic
        .cases
        .iter()
        .any(|case| case.id == "unknown-bob" && case.should_abstain));

    let smoke = parsed
        .iter()
        .find(|d| d.name == "smoke synthetic recall")
        .expect("smoke dataset");
    assert!(smoke.cases.len() >= 10);
    let smoke_aspects: std::collections::HashSet<_> =
        smoke.cases.iter().map(|c| c.aspect.clone()).collect();
    for expected in &["alias", "exact", "keyword_bm25", "abstention"] {
        assert!(
            smoke_aspects.contains(*expected),
            "smoke missing aspect: {expected}"
        );
    }

    let quality = parsed
        .iter()
        .find(|d| d.name == "quality synthetic recall")
        .expect("quality dataset");
    assert!(quality.cases.len() >= 15);
    let quality_aspects: std::collections::HashSet<_> =
        quality.cases.iter().map(|c| c.aspect.clone()).collect();
    for expected in &[
        "alias",
        "exact",
        "fact_graph",
        "keyword_bm25",
        "temporal_update",
        "abstention",
    ] {
        assert!(
            quality_aspects.contains(*expected),
            "quality missing aspect: {expected}"
        );
    }

    let stress = parsed
        .iter()
        .find(|d| d.name == "stress synthetic recall")
        .expect("stress dataset");
    assert!(stress.cases.len() >= 5);

    let temporal = parsed
        .iter()
        .find(|d| d.name == "temporal synthetic recall")
        .expect("temporal dataset");
    assert!(temporal.cases.len() >= 9);

    let adversarial = parsed
        .iter()
        .find(|d| d.name == "adversarial synthetic recall")
        .expect("adversarial dataset");
    assert!(adversarial.cases.len() >= 9);
    assert!(adversarial
        .cases
        .iter()
        .any(|case| case.aspect == "adversarial_alias"));
    assert!(adversarial
        .cases
        .iter()
        .any(|case| case.aspect == "adversarial_negation"));

    for dataset in &parsed {
        for case in &dataset.cases {
            if !case.should_abstain {
                assert!(
                    !case.expected_memory_ids.is_empty(),
                    "dataset {} case {} must have expected_memory_ids",
                    dataset.name,
                    case.id
                );
            }
        }
    }

    Ok(())
}

fn run_dataset_from_file(raw: &str) -> Result<EvalReport> {
    let dataset: EvalDataset = serde_json::from_str(raw)?;
    let temp = TempDir::new()?;
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()))?;
    run_recall_eval(&engine, dataset)
}

fn assert_quality_floor(report: &EvalReport) {
    let gate = evaluate_recall_quality_gate(report, RecallQualityGateProfile::synthetic_quality());
    assert!(
        gate.passed,
        "dataset {} failed quality gate: failures={:?} warnings={:?}",
        report.dataset_name, gate.failures, gate.warnings
    );
}

#[test]
fn synthetic_datasets_run_without_eval_errors() -> Result<()> {
    for raw in [
        include_str!("../../../evals/synthetic/smoke.json"),
        include_str!("../../../evals/synthetic/quality.json"),
        include_str!("../../../evals/synthetic/stress.json"),
        include_str!("../../../evals/synthetic/temporal.json"),
        include_str!("../../../evals/synthetic/adversarial.json"),
    ] {
        let report = run_dataset_from_file(raw)?;
        assert!(report.case_count > 0);
    }
    Ok(())
}

#[test]
fn pr_quality_datasets_meet_hard_gate() -> Result<()> {
    let smoke = run_dataset_from_file(include_str!("../../../evals/synthetic/smoke.json"))?;
    assert_quality_floor(&smoke);

    let quality = run_dataset_from_file(include_str!("../../../evals/synthetic/quality.json"))?;
    assert_quality_floor(&quality);

    let temporal = run_dataset_from_file(include_str!("../../../evals/synthetic/temporal.json"))?;
    assert_quality_floor(&temporal);

    let adversarial =
        run_dataset_from_file(include_str!("../../../evals/synthetic/adversarial.json"))?;
    assert_quality_floor(&adversarial);

    let stress = run_dataset_from_file(include_str!("../../../evals/synthetic/stress.json"))?;
    assert_quality_floor(&stress);
    Ok(())
}

#[test]
fn recall_eval_rejects_unknown_case_memory_ids() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()))?;
    let dataset = EvalDataset {
        name: "bad references".to_string(),
        memories: Vec::new(),
        cases: vec![EvalCase {
            id: "missing-reference".to_string(),
            aspect: "validation".to_string(),
            query: "missing".to_string(),
            expected_memory_ids: vec!["does-not-exist".to_string()],
            forbidden_memory_ids: vec!["also-missing".to_string()],
            limit: 5,
            deep: false,
            should_abstain: false,
            dream_before_recall: false,
        }],
    };

    let error = run_recall_eval(&engine, dataset).expect_err("unknown case memory ids should fail");

    assert!(error.to_string().contains("missing-reference"));
    assert!(error.to_string().contains("does-not-exist"));
    assert!(error.to_string().contains("also-missing"));
    Ok(())
}

#[test]
fn recall_eval_rejects_non_abstention_case_without_expected_ids() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()))?;
    let dataset = EvalDataset {
        name: "missing expectations".to_string(),
        memories: Vec::new(),
        cases: vec![EvalCase {
            id: "not-measurable".to_string(),
            aspect: "validation".to_string(),
            query: "anything".to_string(),
            expected_memory_ids: Vec::new(),
            forbidden_memory_ids: Vec::new(),
            limit: 5,
            deep: false,
            should_abstain: false,
            dream_before_recall: false,
        }],
    };

    let error = run_recall_eval(&engine, dataset).expect_err("case without target should fail");

    assert!(error.to_string().contains("not-measurable"));
    assert!(error.to_string().contains("expected_memory_ids"));
    Ok(())
}

#[test]
fn eval_case_can_run_dream_before_recall_for_conflict_updates() -> Result<()> {
    let temp = TempDir::new()?;
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()))?;
    let dataset = EvalDataset {
        name: "dream update synthetic".to_string(),
        memories: vec![
            EvalMemory {
                id: "alice-profile".to_string(),
                content: "Alice is a person.".to_string(),
                entities: vec![EntityInput {
                    entity_type: "person".to_string(),
                    name: "Alice".to_string(),
                    aliases: Vec::new(),
                    confidence: 0.9,
                    source: ExtractionSource::Manual,
                }],
                facts: Vec::new(),
                session_id: Some("profile".to_string()),
                recorded_at: Some(Utc.with_ymd_and_hms(2026, 4, 19, 9, 0, 0).unwrap()),
                confidence: 0.9,
            },
            fact_memory(
                "alice-paris-old-a",
                "Alice lived in Paris in 2024.",
                "Paris",
                0.70,
                "old-a",
                20,
            ),
            fact_memory(
                "alice-paris-old-b",
                "Alice still used the Paris address in 2024.",
                "Paris",
                0.72,
                "old-b",
                21,
            ),
            fact_memory(
                "alice-london-new-a",
                "Alice moved to London in 2026.",
                "London",
                0.96,
                "new-a",
                22,
            ),
            fact_memory(
                "alice-london-new-b",
                "Alice confirmed London is her current home.",
                "London",
                0.97,
                "new-b",
                23,
            ),
        ],
        cases: vec![EvalCase {
            id: "current-home-after-dream".to_string(),
            aspect: "conflict_invalidation".to_string(),
            query: "Alice current home London".to_string(),
            expected_memory_ids: vec![
                "alice-london-new-a".to_string(),
                "alice-london-new-b".to_string(),
            ],
            forbidden_memory_ids: vec![
                "alice-paris-old-a".to_string(),
                "alice-paris-old-b".to_string(),
            ],
            limit: 5,
            deep: true,
            should_abstain: false,
            dream_before_recall: true,
        }],
    };

    let report = run_recall_eval(&engine, dataset)?;

    assert_eq!(report.case_count, 1);
    assert_eq!(report.recall_at_1, 1.0);
    assert!(report.cases.iter().any(|case| {
        case.id == "current-home-after-dream"
            && case.aspect == "conflict_invalidation"
            && case.dream_before_recall
            && case.expected_hit
            && case.has_forbidden_expectations
            && case.clean_hit
            && case.failure_modes.is_empty()
            && case.forbidden_result_ids.is_empty()
            && case
                .result_ids
                .iter()
                .any(|id| id == "alice-london-new-a" || id == "alice-london-new-b")
    }));
    Ok(())
}

fn fact_memory(
    id: &str,
    content: &str,
    object: &str,
    confidence: f32,
    session_id: &str,
    day: u32,
) -> EvalMemory {
    EvalMemory {
        id: id.to_string(),
        content: content.to_string(),
        entities: Vec::new(),
        facts: vec![FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: object.to_string(),
            confidence,
            source: ExtractionSource::Manual,
        }],
        session_id: Some(session_id.to_string()),
        recorded_at: Some(Utc.with_ymd_and_hms(2026, 4, day, 9, 0, 0).unwrap()),
        confidence,
    }
}
