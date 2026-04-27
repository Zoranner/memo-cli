# Benchmark Evaluation Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** Completed as an implementation snapshot. The current code already contains the eval trace model, dataset tiers, JSON artifacts, baseline compare, quality gate, and normalized public JSONL adapter. This document is kept as historical implementation evidence, not as an active unchecked task list.

**Goal:** 完成 memo-brain benchmark/eval 套件的 trace、指标、数据集分层、JSON 产物、baseline compare 和公开 benchmark adapter 骨架。

**Architecture:** 继续以 `crates/engine/src/eval.rs` 为评测核心，保持 `cargo bench` 和 `recall_eval` example 两个入口。新增指标只读现有 recall 结果，不改变召回排序语义。

**Tech Stack:** Rust, memo-engine, serde_json, Criterion, tempfile, PowerShell validation commands

---

### Task: Expand Eval Report Model

**Files:**
- Modify: `crates/engine/src/eval.rs`
- Modify: `crates/engine/tests/eval_runner.rs`

- [x] Write failing tests for trace, precision, contamination, record kind counts, timing, and failure mode counts.
- [x] Add `EvalResultTrace`, `EvalTimingReport`, `EvalKindCount`, and `EvalFailureModeCount`.
- [x] Compute trace entries from `RecallResult` without changing recall behavior.
- [x] Compute precision, clean precision, forbidden rate, noise hit rate, record kind counts, and failure counts.
- [x] Run `cargo test -p memo-engine --test eval_runner`.

### Task: Add Dataset Tiers

**Files:**
- Create: `evals/synthetic/smoke.json`
- Create: `evals/synthetic/quality.json`
- Create: `evals/synthetic/stress.json`
- Create: `evals/synthetic/temporal.json`
- Create: `evals/synthetic/adversarial.json`
- Modify: `crates/engine/tests/eval_runner.rs`
- Modify: `crates/engine/benches/recall_quality.rs`

- [x] Copy current `basic.json` into the quality tier.
- [x] Add smaller smoke dataset.
- [x] Add stress, temporal, and adversarial datasets with deterministic expected ids.
- [x] Add parsing tests for every dataset.
- [x] Point quality bench at `quality.json`.

### Task: Add JSON Artifact And Compare

**Files:**
- Modify: `crates/engine/examples/recall_eval.rs`
- Modify: `crates/engine/benches/recall_quality.rs`
- Modify: `crates/engine/tests/eval_runner.rs`

- [x] Add `--output <path>` to `recall_eval`.
- [x] Add `--compare <baseline.json>` to `recall_eval`.
- [x] Add compare helpers in `eval.rs`.
- [x] Make `recall_quality` write `target/evals/recall_quality.json`.
- [x] Add tests for compare pass and compare failure.

### Task: Add Public Adapter Skeleton

**Files:**
- Modify: `crates/engine/src/eval.rs`
- Modify: `crates/engine/tests/eval_runner.rs`
- Create: `evals/public/README.md`

- [x] Add normalized public JSONL event structs.
- [x] Convert memory/query events into `EvalDataset`.
- [x] Reject query events without expected ids or abstention marker.
- [x] Document that LongMemEval/LOCOMO concrete parsers require source-format verification before implementation.

### Task: Verify Suite

**Files:**
- Modify only benchmark/eval files and docs listed above.

- [x] Run `cargo fmt --all`.
- [x] Run `cargo test -p memo-engine --test eval_runner`.
- [x] Run `cargo test -p memo-engine --benches --no-run`.
- [x] Run `cargo bench -p memo-engine --bench recall_quality -- --sample-size 10`.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Run `cargo build --all-features`.
- [x] Run `cargo test --workspace --all-features`.
