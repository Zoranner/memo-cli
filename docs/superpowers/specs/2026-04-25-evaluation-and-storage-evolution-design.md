# Evaluation And Storage Evolution Design

**Date:** 2026-04-25

## Goal

把后续开发从“凭感觉优化记忆系统”切换到“用可复现评测和基准数据驱动优化”。同时系统评估数据库访问层是否需要从当前 `rusqlite` 手写 SQL 演进到 ORM 或 typed query 方案，避免在没有效果和性能基线前进行高风险迁移。

## Scope

本设计覆盖三条后续主线：

- 建立性能 benchmark，评估 `remember`、`recall`、`dream`、`restore` 的延迟、候选规模和派生索引成本。
- 建立记忆效果评测，覆盖 raw retrieval、deep recall、rerank、dream 后沉淀效果和拒答能力。
- 建立数据库访问层 spike，对比当前 `rusqlite` repository、Diesel、SQLx 三种路线，再决定是否迁移。

本设计不直接修改线上命令语义，不在第一阶段引入全量 ORM，不把公开 benchmark 跑分作为唯一目标。

## Current Context

当前 engine 的核心边界是 SQLite 真相源、Tantivy 文本索引、HNSW 向量索引、L3 cache 与 session cache。架构文档已经明确“真相只存在 SQLite”，派生层必须可以从真相源恢复。

当前 `benches/` 目录没有实际 benchmark 文件，说明项目缺少持续评估性能和效果的工具。当前数据库层使用 `rusqlite`，SQL 与 row mapping 分布在 `crates/engine/src/db/*`。这套方式可控，但 schema 演进、查询映射错误和复杂查询维护成本会继续上升。

## Design Principles

- Benchmark 先行。任何 ranking、dream、graph expansion、vector index、provider fallback 优化，都应能在本地评测中体现收益或代价。
- 先测 raw retrieval。不要一开始用 LLM 生成最终答案掩盖召回问题。
- 真相源优先。数据库访问层演进不能破坏 SQLite 是唯一业务真相源的架构边界。
- ORM 后判定。先用真实表、真实查询、真实迁移样例做 spike，再决定是否引入 ORM。
- 可复现优先。评测数据、运行命令、输出指标必须可以被提交到仓库中复跑。

## Benchmark Design

新增 `benches/` 下的 Criterion 基准，第一批只评估本地可复现路径，不依赖外部 provider。

建议基准项：

- `remember_manual_episode`
- `recall_exact_alias_fast`
- `recall_bm25_fast`
- `recall_graph_expansion`
- `recall_mmr_selection`
- `dream_rule_based_pass`
- `restore_text_index_full`
- `restore_vector_index_full_with_existing_vectors`

数据规模分为 small、medium、large 三档。small 用于开发时快速反馈，medium 用于 PR 前检查，large 用于手动性能分析。

输出至少记录：

- wall-clock latency
- total candidates
- selected results
- SQLite row count
- text index document count
- vector index document count
- benchmark data set name

## Evaluation Design

新增 `evals/` 目录，用于存放记忆质量评测数据和 runner。第一阶段使用 synthetic 数据，因为它更容易固定 expected memory id 和失败原因。

评测类型：

- exact recall：直接事实是否能命中。
- alias recall：别名、简称、大小写归一是否能命中同一实体。
- multi-hop recall：通过 entity/fact/edge graph expansion 找到间接关系。
- temporal recall：同一主体的旧事实与新事实能否正确处理。
- knowledge update：新事实覆盖旧事实后，dream 是否能保留正确事实并失效冲突事实。
- abstention：没有证据时是否能返回低置信或空结果，而不是凑答案。
- provider fallback：embedding 或 rerank 失败时，非向量路径是否仍可用。

核心指标：

- recall@1
- recall@5
- MRR
- expected memory hit rate
- duplicate rate in selected results
- abstention correctness
- latency p50 and p95

第二阶段再接公开数据集。优先 LongMemEval，因为它覆盖长期记忆、多会话推理、知识更新、时间推理和拒答能力，更贴近本项目目标。后续再评估 LOCOMO、BEAM 或其他 memory benchmark。

## Public Benchmark Strategy

公开 benchmark 不应只追最终分数。每次跑分报告应拆开记录：

- fast recall only
- deep recall without rerank
- deep recall with rerank
- dream before recall
- graph expansion enabled or disabled
- provider type and model
- total latency and provider call count
- token or API cost

这样可以判断项目设计中每个模块是否真实贡献效果，而不是只得到一个不可解释的总分。

## Storage Evolution Design

数据库访问层先做 spike，不直接全量迁移。

候选路线：

- `rusqlite` repository hardening：保留当前同步 SQLite 访问方式，重点补强 migration、row mapper、query boundary 和测试。
- Diesel：引入更强类型约束和 migration 体系，适合当前同步 CLI/engine 模型，但迁移成本较高。
- SQLx：保留 SQL 可见性，引入 typed query 与 migration 能力，可能比完整 ORM 更贴合复杂检索查询。

SeaORM 暂不作为第一候选。它更偏 async service 形态，可能把当前同步 engine 边界改大，收益不一定覆盖迁移成本。

## Spike Acceptance Criteria

数据库 spike 必须用真实问题验证，不做空泛技术选型。

最小样例应覆盖：

- schema migration，从当前 schema 迁移到带 `PRAGMA user_version` 的版本化迁移。
- episode insert 和 read。
- entity alias upsert 和 exact alias search。
- fact insert 和 conflict query。
- memory layer update。
- index job enqueue 和 consume。

每条路线都要记录：

- 改动文件范围。
- 编译时间和二进制依赖变化。
- SQL 可读性。
- 类型安全收益。
- migration 复杂度。
- 对现有 `MemoryEngine` API 的侵入程度。
- 对 benchmark 结果的影响。

只有当收益明确超过迁移风险时，才进入正式迁移计划。

## Development Order

- 稳定验证基线：确认 `cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --all-features` 能跑通，或明确记录网络/DNS 阻断。
- Benchmark 基础设施：引入 Criterion，先覆盖本地无 provider 的核心路径。
- Evaluation runner：建立 synthetic dataset 与 recall quality metrics。
- Schema versioning：补 `PRAGMA user_version` 迁移体系。
- Storage spike：比较 `rusqlite` repository、Diesel、SQLx。
- Vector persistence：改进 vector index 原子写入。
- Recall tests and ranking optimization：补窄测试，再优化 MMR 与 token 缓存。
- Dream decomposition：在评测保护下拆分 dream 模块。
- Public benchmark adapter：接 LongMemEval 并输出可解释报告。

## Verification

文档阶段不要求执行 Rust 门禁。进入代码阶段后，每个阶段至少执行：

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- 阶段相关测试，例如 `cargo test -p memo-engine --test engine_flow`
- benchmark 或 eval runner 的窄命令

公开 benchmark adapter 阶段还需要保存一份可复现运行记录，包含数据集版本、provider 配置摘要、命令、指标和失败样例。

## Open Decisions

- Criterion benchmark 是否进入默认 CI，还是只作为手动性能检查。
- `evals/` runner 使用 Rust CLI、Python 脚本还是二者结合。
- LongMemEval adapter 是否允许在线 provider，还是先做纯本地 raw retrieval 评测。
- storage spike 是否单独开分支，避免和 schema migration 主线互相污染。
