# Benchmark Evaluation Suite Design

**Date:** 2026-04-26

## Goal

把当前 recall benchmark 从“能给整体分数”升级为“能解释问题、能比较趋势、能分层运行、能接入更大数据集”的评测套件。

## Scope

本轮覆盖十个目标：

- 结果解释 trace。
- synthetic 数据集分层扩容。
- 阶段耗时拆分。
- record-type 维度指标。
- precision、contamination、duplicate 指标。
- 固定 JSON 输出产物。
- baseline compare。
- deterministic vector/embedding 评测基础。
- 公开 benchmark adapter 骨架。
- CI 分层命令与文档。

本轮不改变召回排序行为，不把公开数据格式硬编码为未验证假设，不新增稳定 `memo eval` CLI 命令。`cargo bench` 仍是质量 benchmark 主入口，`cargo run -p memo-engine --example recall_eval` 作为人类可读和 JSON 产物入口。

## Architecture

`crates/engine/src/eval.rs` 继续作为评测核心，扩展报告模型而不是新增平行 runner。报告分为四层：

- `EvalReport`：全局聚合指标、阶段耗时、数据集名和 case 列表。
- `EvalAspectReport`：按 aspect 聚合指标。
- `EvalCaseReport`：单 case 指标、失败模式、结果 id 和 trace。
- `EvalResultTrace`：单条 recall result 的解释信息。

trace 第一阶段从现有 `RecallResult` 可见信息构建，包括 record kind、layer、score、rank、source memory id、recall reasons、expected/forbidden/duplicate 标记。当前 recall pipeline 已经透出 `RecallReason`，因此不需要先改检索核心。

数据集目录改为分层：

- `evals/synthetic/smoke.json`：快速冒烟。
- `evals/synthetic/quality.json`：主质量 bench。
- `evals/synthetic/stress.json`：噪声和重复压力。
- `evals/synthetic/temporal.json`：当前状态、历史事实、冲突。
- `evals/synthetic/adversarial.json`：相似实体、歧义、否定和污染。
- `evals/public/README.md`：公开 benchmark adapter 约束和后续格式验证入口。

## Metrics

保留现有指标：

- `recall_at_1`
- `recall_at_5`
- `source_recall_at_1`
- `source_recall_at_5`
- `mrr`
- `source_mrr`
- `expected_hit_rate`
- `clean_hit_rate`
- `successful_case_rate`
- `mean_source_diversity`
- `mean_duplicate_rate`
- `abstention_correctness`
- `forbidden_correctness`

新增指标：

- `precision_at_1`
- `precision_at_5`
- `clean_precision_at_5`
- `forbidden_rate`
- `noise_hit_rate`
- record kind distribution
- failure mode distribution
- timing summary

`precision_at_k` 只在有 expected memory 的 case 上统计。`clean_precision_at_5` 从 precision@5 中扣除 forbidden 命中。`forbidden_rate` 衡量结果中 forbidden source 占比。`noise_hit_rate` 衡量既不是 expected 也不是 forbidden 的 source 占比。

## Timing

评测 runner 记录：

- load memory time
- initial restore time
- dream time
- recall time
- total time
- per-case recall time

Criterion 继续负责 wall-clock benchmark；eval timing 负责语义阶段拆分。

## Output

`recall_eval` example 支持：

- human output
- `--json`
- `--output <path>`
- `--compare <baseline.json>`

`recall_quality` bench 每次写入：

- `target/evals/recall_quality.json`

baseline compare 对以下指标做保守阈值：

- recall、clean hit、success 不允许明显下降。
- forbidden、duplicate、latency 不允许明显上升。
- compare 失败直接返回 error，方便 CI 使用。

## External Benchmark Adapter

公开 benchmark 先保留 adapter 骨架，不硬编码具体 LongMemEval 或 LOCOMO 字段。原因是公开数据集格式可能变化，必须在接入时联网验证原始数据说明。

第一阶段 adapter 接受仓库定义的 normalized JSONL：

- `memory` 事件转换为 `EvalMemory`。
- `query` 事件转换为 `EvalCase`。
- 每条 query 必须提供 expected 或 abstention 标记。

后续接 LongMemEval/LOCOMO 时，只新增具体 parser，不改变 `EvalDataset` 和 runner。

## CI Strategy

- PR：跑 smoke eval、eval runner tests、普通 Rust tests。
- Nightly：跑 quality、temporal、adversarial、stress。
- Manual：跑 public adapter 和大规模数据。

## Verification

每轮代码完成后执行：

- `cargo fmt --all`
- `cargo test -p memo-engine --test eval_runner`
- `cargo test -p memo-engine --benches --no-run`
- `cargo bench -p memo-engine --bench recall_quality -- --sample-size 10`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build --all-features`
- `cargo test --workspace --all-features`
