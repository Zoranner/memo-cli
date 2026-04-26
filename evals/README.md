# Evaluation And Benchmarking

This repository uses `cargo bench` as the primary benchmark entry. The evaluation
runner and datasets are shared so quality checks, latency checks, and diagnostic
reports measure the same behavior.

## Directory Layout

- `evals/synthetic/*.json`: source datasets grouped by cost and failure mode.
- `evals/public/`: normalized public benchmark adapter notes and future inputs.
- `crates/engine/src/eval.rs`: reusable evaluation model, metrics, quality gate,
  baseline comparison, and normalized public JSONL adapter.
- `crates/engine/benches/recall_quality.rs`: Criterion quality suite. It runs the
  synthetic quality dataset, applies the formal quality gate, prints warnings for
  known retrieval risks, and writes a JSON report.
- `crates/engine/benches/recall_latency.rs`: Criterion microbench suite for core
  recall paths and remember latency.
- `crates/engine/examples/recall_eval.rs`: one-shot diagnostic runner for
  human-readable reports, JSON artifacts, and baseline comparison.
- `crates/engine/benches/support/quality.rs`: quality bench fixtures, report
  artifact writing, and quality report printing.
- `crates/engine/benches/support/latency.rs`: latency bench fixtures and
  deterministic embedding setup.

## Quality Gate

The synthetic quality bench enforces required floors through
`RecallQualityGateProfile::synthetic_quality()`:

- `recall_at_1 >= 0.60`
- `recall_at_5 >= 0.85`
- `mrr >= 0.70`
- `source_mrr >= 0.70`
- `clean_hit_rate >= 0.50`
- `successful_case_rate >= 0.75`
- `abstention_correctness >= 1.0`
- `forbidden_correctness >= 0.50`
- `mean_duplicate_rate <= 0.30`

Known retrieval risks are reported as warnings instead of hidden:

- `forbidden_correctness < 1.0`
- `mean_duplicate_rate > 0.20`

The current benchmark intentionally exposes these risks so ranking fixes can be
measured instead of masked.

## Smoke

`evals/synthetic/smoke.json` is the PR-scale dataset. It checks exact recall, alias recall, and abstention.

Recommended command:

```powershell
cargo run -p memo-engine --example recall_eval -- evals\synthetic\smoke.json --json --output target\evals\smoke.json
```

## Quality

`evals/synthetic/quality.json` is the main `cargo bench` quality dataset. It
covers exact, alias, graph, BM25, temporal updates, dream-before-recall, deep
recall, and abstention.

Recommended command:

```powershell
cargo bench -p memo-engine --bench recall_quality -- --sample-size 10
```

The bench writes `target/evals/recall_quality.json`.

## Latency

`crates/engine/benches/recall_latency.rs` measures the hot paths separately from
quality scoring:

- `remember_manual_episode`
- `recall_exact_alias_fast`
- `recall_bm25_fast`
- `recall_graph_expansion`
- `recall_deep_current_state_after_dream`
- `recall_vector_semantic_deterministic`

Recommended command:

```powershell
cargo bench -p memo-engine --bench recall_latency -- --sample-size 10
```

Run quality and latency benches serially on Windows to avoid Cargo build
directory lock contention.

## Diagnostic Runner

`recall_eval` is a Cargo example, not a Criterion bench target and not a
user-facing `memo` command. It runs a dataset once and writes a detailed report
for developer diagnosis.

Default command:

```powershell
cargo run -p memo-engine --example recall_eval
```

By default it reads `evals/synthetic/quality.json` and writes
`target/evals/recall_eval.json`. Pass an explicit dataset and output path for
focused diagnosis:

```powershell
cargo run -p memo-engine --example recall_eval -- evals\synthetic\temporal.json --json --output target\evals\temporal.json
```

## Stress

`evals/synthetic/stress.json` adds noisy memories and similar terms to measure contamination and duplicate pressure.

## Temporal

`evals/synthetic/temporal.json` isolates current-state recall against old historical memories.

## Adversarial

`evals/synthetic/adversarial.json` covers similar names, misleading negation, and forbidden contamination.

## CI Tiers

PR tier:

```powershell
cargo test -p memo-engine --test eval_runner
cargo run -p memo-engine --example recall_eval -- evals\synthetic\smoke.json --json --output target\evals\smoke.json
```

Nightly tier:

```powershell
cargo run -p memo-engine --example recall_eval -- evals\synthetic\quality.json --json --output target\evals\quality.json
cargo run -p memo-engine --example recall_eval -- evals\synthetic\temporal.json --json --output target\evals\temporal.json
cargo run -p memo-engine --example recall_eval -- evals\synthetic\adversarial.json --json --output target\evals\adversarial.json
cargo run -p memo-engine --example recall_eval -- evals\synthetic\stress.json --json --output target\evals\stress.json
```

Manual tier:

```powershell
cargo bench -p memo-engine --bench recall_quality -- --sample-size 10
cargo bench -p memo-engine --bench recall_latency -- --sample-size 10
cargo run -p memo-engine --example recall_eval
```

Baseline comparison:

```powershell
cargo run -p memo-engine --example recall_eval -- evals\synthetic\quality.json --compare target\evals\baseline-quality.json --output target\evals\quality.json
```
