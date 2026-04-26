use std::{env, path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use memo_engine::{
    eval::{compare_eval_reports, run_recall_eval, EvalCompareOptions, EvalDataset, EvalReport},
    EngineConfig, MemoryEngine,
};
use tempfile::TempDir;

const DEFAULT_DATASET_PATH: &str = "evals/synthetic/quality.json";
const DEFAULT_OUTPUT_PATH: &str = "target/evals/recall_eval.json";

fn main() -> Result<()> {
    let args = Args::parse_from(env::args().skip(1))?;
    let raw = std::fs::read_to_string(&args.dataset_path).with_context(|| {
        format!(
            "failed to read eval dataset {}",
            args.dataset_path.display()
        )
    })?;
    let dataset: EvalDataset = serde_json::from_str(&raw).with_context(|| {
        format!(
            "failed to parse eval dataset {}",
            args.dataset_path.display()
        )
    })?;

    let temp = TempDir::new().context("failed to create temporary eval data directory")?;
    let engine = MemoryEngine::open(EngineConfig::new(temp.path()))?;
    let started_at = Instant::now();
    let report = run_recall_eval(&engine, dataset)?;
    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    if let Some(compare_path) = &args.compare_path {
        let baseline = read_report(compare_path)?;
        let comparison = compare_eval_reports(&baseline, &report, EvalCompareOptions::default());
        if !comparison.passed {
            for regression in &comparison.regressions {
                eprintln!(
                    "Regression: {} baseline={:.3} current={:.3} delta={:.3}",
                    regression.metric, regression.baseline, regression.current, regression.delta
                );
            }
            anyhow::bail!(
                "eval comparison failed with {} regressions",
                comparison.regressions.len()
            );
        }
    }

    write_json_report(&args.output_path, &report)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human_report(&report, elapsed_ms);
    }

    Ok(())
}

struct Args {
    dataset_path: PathBuf,
    json: bool,
    output_path: PathBuf,
    compare_path: Option<PathBuf>,
}

impl Args {
    fn parse_from(raw_args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut dataset_path = None;
        let mut json = false;
        let mut output_path = workspace_path(DEFAULT_OUTPUT_PATH);
        let mut compare_path = None;
        let mut args = raw_args.into_iter().peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bench" => {}
                "--json" => json = true,
                "--output" => {
                    let Some(path) = args.next() else {
                        anyhow::bail!("--output requires a path");
                    };
                    output_path = resolve_path(PathBuf::from(path));
                }
                "--compare" => {
                    let Some(path) = args.next() else {
                        anyhow::bail!("--compare requires a path");
                    };
                    compare_path = Some(resolve_path(PathBuf::from(path)));
                }
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ if dataset_path.is_none() => dataset_path = Some(PathBuf::from(arg)),
                _ => anyhow::bail!("unexpected argument: {}", arg),
            }
        }

        Ok(Self {
            dataset_path: dataset_path
                .map(resolve_path)
                .unwrap_or_else(|| workspace_path(DEFAULT_DATASET_PATH)),
            json,
            output_path,
            compare_path,
        })
    }
}

fn print_usage() {
    eprintln!(
        "Usage: cargo bench -p memo-engine --bench recall_eval -- [dataset.json] [--json] [--output <report.json>] [--compare <baseline.json>]"
    );
    eprintln!("Defaults: dataset={DEFAULT_DATASET_PATH} output={DEFAULT_OUTPUT_PATH}");
}

fn write_json_report(path: &PathBuf, report: &EvalReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(report)?;
    std::fs::write(path, payload).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn read_report(path: &PathBuf) -> Result<EvalReport> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read baseline {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse baseline {}", path.display()))
}

fn resolve_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        workspace_root().join(path)
    }
}

fn workspace_path(path: &str) -> PathBuf {
    workspace_root().join(path)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("engine crate should live below workspace root")
        .to_path_buf()
}

fn print_human_report(report: &EvalReport, elapsed_ms: f64) {
    println!("Dataset: {}", report.dataset_name);
    println!("Cases: {}", report.case_count);
    println!("Elapsed: {:.2} ms", elapsed_ms);
    println!("Recall@1: {:.3}", report.recall_at_1);
    println!("Recall@5: {:.3}", report.recall_at_5);
    println!("Source recall@1: {:.3}", report.source_recall_at_1);
    println!("Source recall@5: {:.3}", report.source_recall_at_5);
    println!("MRR: {:.3}", report.mrr);
    println!("Source MRR: {:.3}", report.source_mrr);
    println!("Expected hit rate: {:.3}", report.expected_hit_rate);
    println!("Clean hit rate: {:.3}", report.clean_hit_rate);
    println!("Successful case rate: {:.3}", report.successful_case_rate);
    println!("Precision@1: {:.3}", report.precision_at_1);
    println!("Precision@5: {:.3}", report.precision_at_5);
    println!("Clean precision@5: {:.3}", report.clean_precision_at_5);
    println!("Forbidden rate: {:.3}", report.forbidden_rate);
    println!("Noise hit rate: {:.3}", report.noise_hit_rate);
    println!("Mean source diversity: {:.3}", report.mean_source_diversity);
    println!("Mean duplicate rate: {:.3}", report.mean_duplicate_rate);
    println!(
        "Abstention correctness: {:.3}",
        report.abstention_correctness
    );
    println!("Forbidden correctness: {:.3}", report.forbidden_correctness);
    println!("Eval total: {:.2} ms", report.timing.total_ms);
    println!();
    println!("Aspects:");
    for aspect in &report.aspects {
        println!(
            "- {} | cases={} | recall@1={:.3} | recall@5={:.3} | source_recall@1={:.3} | source_recall@5={:.3} | mrr={:.3} | source_mrr={:.3} | clean={:.3} | success={:.3} | precision@5={:.3} | forbidden_rate={:.3} | diversity={:.3} | duplicate={:.3} | abstention={:.3} | forbidden={:.3}",
            aspect.aspect,
            aspect.case_count,
            aspect.recall_at_1,
            aspect.recall_at_5,
            aspect.source_recall_at_1,
            aspect.source_recall_at_5,
            aspect.mrr,
            aspect.source_mrr,
            aspect.clean_hit_rate,
            aspect.successful_case_rate,
            aspect.precision_at_5,
            aspect.forbidden_rate,
            aspect.mean_source_diversity,
            aspect.mean_duplicate_rate,
            aspect.abstention_correctness,
            aspect.forbidden_correctness
        );
    }
    println!();
    println!("Cases:");
    for case in &report.cases {
        let rank = case
            .matched_rank
            .map(|rank| rank.to_string())
            .unwrap_or_else(|| "none".to_string());
        let source_rank = case
            .matched_unique_rank
            .map(|rank| rank.to_string())
            .unwrap_or_else(|| "none".to_string());
        println!(
            "- {} | aspect={} | rank={} | source_rank={} | clean={} | success={} | diversity={:.3} | duplicate={:.3} | duplicates={} | results={} | forbidden_hits={} | failures=[{}] | recall_ms={:.2} | abstention_correct={} | dream={} | deep={} | result_ids=[{}] | unique_ids=[{}] | forbidden_ids=[{}]",
            case.id,
            case.aspect,
            rank,
            source_rank,
            case.clean_hit,
            case.successful,
            case.source_diversity,
            case.duplicate_rate,
            case.duplicate_result_count,
            case.result_count,
            case.forbidden_hit_count,
            case.failure_modes.join(","),
            case.timing_ms,
            case.abstention_correct,
            case.dream_before_recall,
            case.deep_search_used,
            case.result_ids.join(","),
            case.unique_result_ids.join(","),
            case.forbidden_result_ids.join(",")
        );
    }
}
