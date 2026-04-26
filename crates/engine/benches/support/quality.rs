use memo_engine::{
    eval::{run_recall_eval, EvalDataset, EvalReport},
    EngineConfig, MemoryEngine,
};
use tempfile::TempDir;

pub const SYNTHETIC_QUALITY: &str = include_str!("../../../../evals/synthetic/quality.json");

pub fn parse_eval_dataset(raw: &str) -> EvalDataset {
    serde_json::from_str(raw).expect("synthetic eval dataset should parse")
}

pub fn run_eval_dataset(dataset: EvalDataset) -> EvalReport {
    let temp = TempDir::new().expect("temp dir");
    let engine = MemoryEngine::open(EngineConfig::new(temp.path())).expect("open engine");
    run_recall_eval(&engine, dataset).expect("run recall eval")
}
