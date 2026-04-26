use std::path::{Path, PathBuf};

use memo_engine::{
    eval::{
        evaluate_recall_quality_gate, run_recall_eval, EvalDataset, EvalReport,
        RecallQualityGateProfile,
    },
    EngineConfig, MemoryEngine,
};
use tempfile::TempDir;

pub const SYNTHETIC_QUALITY: &str = include_str!("../../../../evals/synthetic/quality.json");
pub const SYNTHETIC_SMOKE: &str = include_str!("../../../../evals/synthetic/smoke.json");

pub fn parse_eval_dataset(raw: &str) -> EvalDataset {
    serde_json::from_str(raw).expect("synthetic eval dataset should parse")
}

pub fn run_eval_dataset(dataset: EvalDataset) -> EvalReport {
    let temp = TempDir::new().expect("temp dir");
    let engine = MemoryEngine::open(EngineConfig::new(temp.path())).expect("open engine");
    run_recall_eval(&engine, dataset).expect("run recall eval")
}

pub fn check_quality_gate(report: &EvalReport, emit_warnings: bool) -> bool {
    let gate = evaluate_recall_quality_gate(report, RecallQualityGateProfile::synthetic_quality());
    if emit_warnings {
        for warning in &gate.warnings {
            eprintln!(
                "recall_quality warning profile={} metric={} expected={:.3} actual={:.3} message={}",
                gate.profile, warning.metric, warning.expected, warning.actual, warning.message
            );
        }
    }
    for failure in &gate.failures {
        eprintln!(
            "recall_quality failure profile={} metric={} expected={:.3} actual={:.3} message={}",
            gate.profile, failure.metric, failure.expected, failure.actual, failure.message
        );
    }
    gate.passed
}

pub fn write_eval_report_artifact(name: &str, report: &EvalReport) {
    let path = workspace_root().join(format!("target/evals/{name}.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create eval artifact directory");
    }
    let payload = serde_json::to_string_pretty(report).expect("serialize eval report");
    std::fs::write(path, payload).expect("write eval report artifact");
}

pub fn print_recall_quality_report(report: &EvalReport) {
    eprintln!(
        "recall_quality baseline dataset={} cases={} recall@1={:.3} recall@5={:.3} source_recall@1={:.3} source_recall@5={:.3} mrr={:.3} source_mrr={:.3} hit_rate={:.3} clean_hit={:.3} success={:.3} precision@1={:.3} precision@5={:.3} clean_precision@5={:.3} diversity={:.3} duplicate_rate={:.3} forbidden_rate={:.3} noise={:.3} abstention={:.3} forbidden={:.3} total_ms={:.2}",
        report.dataset_name,
        report.case_count,
        report.recall_at_1,
        report.recall_at_5,
        report.source_recall_at_1,
        report.source_recall_at_5,
        report.mrr,
        report.source_mrr,
        report.expected_hit_rate,
        report.clean_hit_rate,
        report.successful_case_rate,
        report.precision_at_1,
        report.precision_at_5,
        report.clean_precision_at_5,
        report.mean_source_diversity,
        report.mean_duplicate_rate,
        report.forbidden_rate,
        report.noise_hit_rate,
        report.abstention_correctness,
        report.forbidden_correctness,
        report.timing.total_ms
    );
    for aspect in &report.aspects {
        eprintln!(
            "recall_quality aspect={} cases={} recall@1={:.3} recall@5={:.3} mrr={:.3} clean={:.3} success={:.3} forbidden_rate={:.3} duplicate_rate={:.3}",
            aspect.aspect,
            aspect.case_count,
            aspect.recall_at_1,
            aspect.recall_at_5,
            aspect.mrr,
            aspect.clean_hit_rate,
            aspect.successful_case_rate,
            aspect.forbidden_rate,
            aspect.mean_duplicate_rate
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("engine crate should live below workspace root")
        .to_path_buf()
}
