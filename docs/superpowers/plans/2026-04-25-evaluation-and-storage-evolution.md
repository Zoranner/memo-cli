# Evaluation And Storage Evolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 memo-brain 的第一套可复现性能 benchmark 与记忆质量 eval 基础设施，并为后续数据库访问层 spike 留出明确边界。

**Architecture:** 第一阶段只实现本地无 provider 路径。`crates/engine/benches` 使用 Criterion 评估 engine 热路径；`crates/engine/src/eval.rs` 提供可测试的 evaluation runner；`crates/engine/tests/eval_runner.rs` 锁定指标计算行为；`evals/synthetic/basic.json` 提供仓库内可复现样例。数据库 ORM/SQLx/Diesel 迁移不在本切片实现，只在计划和后续 spike 中处理。

**Tech Stack:** Rust workspace, memo-engine, rusqlite, Tantivy, HNSW, Criterion, serde_json, tempfile

---

### Task: Add Evaluation Runner

**Files:**
- Modify: `crates/engine/src/lib.rs`
- Create: `crates/engine/src/eval.rs`
- Create: `crates/engine/tests/eval_runner.rs`
- Create: `evals/synthetic/basic.json`

- [x] **Step: Write failing eval metrics test**

Create `crates/engine/tests/eval_runner.rs` with a test that builds a temporary `MemoryEngine`, writes synthetic memories, runs eval cases, and asserts `recall_at_1`, `recall_at_5`, `mrr`, `expected_hit_rate`, and `abstention_correctness`.

- [x] **Step: Run test to verify it fails**

Run: `cargo test -p memo-engine --test eval_runner`

Expected: compile failure because `memo_engine::eval` does not exist.

- [x] **Step: Implement eval module**

Create `crates/engine/src/eval.rs` with serializable `EvalDataset`, `EvalCase`, `EvalReport`, `EvalCaseReport`, and `run_recall_eval`. The runner should insert dataset memories through `MemoryEngine::remember`, optionally call `restore_full`, run `recall`, and compute deterministic metrics from expected memory ids.

- [x] **Step: Export eval API**

Modify `crates/engine/src/lib.rs` to expose the eval module behind normal engine builds.

- [x] **Step: Run eval test to verify it passes**

Run: `cargo test -p memo-engine --test eval_runner`

Expected: pass.

### Task: Add Synthetic Dataset

**Files:**
- Create: `evals/synthetic/basic.json`

- [x] **Step: Add JSON dataset**

Add a small synthetic dataset covering exact, alias, graph, and abstention cases. The dataset should be readable by future CLI runners and also document the expected ids in plain JSON.

- [x] **Step: Add dataset parsing test**

Extend `crates/engine/tests/eval_runner.rs` with `synthetic_dataset_file_is_valid`, using `include_str!("../../../evals/synthetic/basic.json")` and `serde_json::from_str`.

- [x] **Step: Run dataset parsing test**

Run: `cargo test -p memo-engine --test eval_runner synthetic_dataset_file_is_valid`

Expected: pass.

### Task: Add Criterion Benchmarks

**Files:**
- Modify: `crates/engine/Cargo.toml`
- Create: `crates/engine/benches/recall_latency.rs`

- [x] **Step: Add Criterion dependency and bench target**

Add `criterion = "0.5"` to `crates/engine` dev-dependencies and register `[[bench]] name = "recall_latency" harness = false`.

- [x] **Step: Implement first benchmark**

Create `recall_latency.rs` with temp-dir-backed engine setup and benchmarks for manual remember, exact alias recall, BM25 recall after restore, and graph recall.

- [x] **Step: Run benchmark compile check**

Run: `cargo bench -p memo-engine --bench recall_latency --no-run`

Expected: benchmark target compiles.

### Task: Verify Baseline

**Files:**
- Modify only files from the tasks above.

- [x] **Step: Format**

Run: `cargo fmt --all`

Expected: success.

- [x] **Step: Run narrow tests**

Run: `cargo test -p memo-engine --test eval_runner`

Expected: success.

- [x] **Step: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Expected: success, or a clearly reported environment/dependency failure.

- [x] **Step: Run full tests if dependencies are available**

### Task: Add Schema Versioning

**Files:**
- Modify: `crates/engine/src/db/schema.rs`
- Modify: `crates/engine/src/db/tests.rs`

- [x] **Step: Write failing schema version tests**

Add tests that require `Database::open` to set `PRAGMA user_version`, upgrade legacy schema version `0`, and reject databases with a future schema version.

- [x] **Step: Run schema tests to verify they fail**

Run: `cargo test -p memo-engine db::tests::open_`

Expected: fail because the database currently leaves `PRAGMA user_version` at `0` and does not reject future versions.

- [x] **Step: Implement schema version migration boundary**

Add a current schema version constant, guard against future versions, route version `0` databases through the existing additive migration logic, and persist the current `PRAGMA user_version`.

- [x] **Step: Run schema tests to verify they pass**

Run: `cargo test -p memo-engine db::tests::open_`

Expected: pass.

### Task: Add Rusqlite Storage Spike Baseline

**Files:**
- Modify: `crates/engine/src/db/tests.rs`

- [x] **Step: Add current storage contract test**

Add a `rusqlite_storage_baseline_covers_spike_acceptance_flow` characterization test covering the storage spike acceptance flow: schema-opened database, episode insert/read, entity alias upsert and exact alias search, fact conflict candidates, memory layer update, index job enqueue, and index job consume.

- [x] **Step: Run baseline test**

Run: `cargo test -p memo-engine db::tests::rusqlite_storage_baseline_covers_spike_acceptance_flow -- --nocapture`

Expected: pass. This test becomes the behavior contract future SQLx/Diesel experiments must preserve.

### Task: Add Cargo Eval Report Runner

**Files:**
- Create: `crates/engine/examples/recall_eval.rs`

- [x] **Step: Add cargo-runnable eval example**

Add a `memo-engine` example runner that reads an eval dataset JSON file, runs `run_recall_eval` against a temporary local engine, and prints human-readable metrics by default.

- [x] **Step: Add JSON report mode**

Support `--json` so the same runner can be consumed by scripts or future CI jobs without adding a stable `memo` CLI command yet.

- [x] **Step: Run synthetic eval report**

Run: `cargo run -p memo-engine --example recall_eval -- evals/synthetic/basic.json`

Observed baseline:

- `Recall@1`: `0.750`
- `Recall@5`: `0.750`
- `MRR`: `0.750`
- `Expected hit rate`: `0.750`
- `Abstention correctness`: `1.000`

- [x] **Step: Run synthetic eval JSON report**

Run: `cargo run -p memo-engine --example recall_eval -- evals/synthetic/basic.json --json`

Expected: emits a serialized `EvalReport`.

### Task: Add Cargo Bench Quality Evaluation

**Files:**
- Modify: `crates/engine/Cargo.toml`
- Modify: `crates/engine/src/eval.rs`
- Modify: `crates/engine/tests/eval_runner.rs`
- Modify: `crates/engine/examples/recall_eval.rs`
- Modify: `evals/synthetic/basic.json`
- Create: `crates/engine/benches/recall_quality.rs`

- [x] **Step: Add aspect-level eval reporting**

Add `aspect` to eval cases and report per-aspect metrics so quality evaluation covers separate recall capabilities instead of only a single aggregate score.

- [x] **Step: Expand synthetic dataset aspects**

Cover `exact`, `alias`, `fact_graph`, `keyword_bm25`, and `abstention` in `evals/synthetic/basic.json`.

- [x] **Step: Add cargo bench quality target**

Register `[[bench]] name = "recall_quality"` and add a Criterion benchmark that runs the full synthetic eval, asserts baseline quality floors, and prints aggregate plus per-aspect quality metrics.

- [x] **Step: Run quality bench**

Run: `cargo bench -p memo-engine --bench recall_quality -- --sample-size 10`

Observed baseline:

- `Cases`: `9`
- `Recall@1`: `0.667`
- `Recall@5`: `0.889`
- `MRR`: `0.759`
- `Expected hit rate`: `0.889`
- `Abstention correctness`: `1.000`
- `Forbidden correctness`: `0.000`
- Criterion time: `[675.07 ms 776.88 ms 895.46 ms]`

Aspect baseline:

- `exact`: `Recall@1 1.000`, `MRR 1.000`
- `alias`: `Recall@1 1.000`, `MRR 1.000`
- `fact_graph`: `Recall@1 1.000`, `MRR 1.000`
- `keyword_bm25`: `Recall@1 1.000`, `MRR 1.000`
- `abstention`: `Abstention correctness 1.000`
- `temporal_update`: `Recall@1 1.000`, `MRR 1.000`, `Forbidden correctness 0.000`
- `conflict_invalidation`: `Recall@1 1.000`, `MRR 1.000`, `Forbidden correctness 0.000`
- `dream_before_after`: `Recall@1 0.000`, `Recall@5 1.000`, `MRR 0.333`, `Forbidden correctness 0.000`
- `deep_recall`: `Recall@1 0.000`, `Recall@5 1.000`, `MRR 0.500`, `Forbidden correctness 0.000`

Interpretation:

- The expanded aspect eval still recalls the expected current London memories within top 5.
- Ranking is weak for `deep_recall` and `dream_before_after`.
- Forbidden old source episodes still appear in current-location recall. This does not mean conflict invalidation failed at the fact/edge layer, but it exposes that raw historical episodes can still surface when the user asks for current state.

Run: `cargo test --all-features`

Expected: success, or a clearly reported environment/dependency failure.
