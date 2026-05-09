# memo-brain 最终标准设计

**日期：** 2026-05-07

**范围：** 项目定位、workspace 架构、公开命令、SQLite 真相源、派生层、provider 边界、记忆分层、Working Set、Pinned、recall、dream、state、评估、bench、测试、文档收敛。

## 文档目的

本文是 memo-brain 的项目级总设计基线。它必须同时覆盖两件事：

- 当前代码已经做了什么；
- 后续最终标准应收敛成什么。

因此本文不是某个局部功能方案，也不是只讨论 Pinned 或数据表的补丁文档。后续如果 README、命令文档、旧架构文档或代码实现与本文冲突，应以本文作为改造依据；但对外用户文档不得把“最终目标”写成“当前已实现能力”。

本文中的“当前实现”来自现有代码结构，核心参考包括：

- CLI：`src/cli/*`
- 配置：`src/config/*`
- provider runtime/status：`src/providers/*`
- engine：`crates/engine/src/*`
- lmkit：`crates/lmkit/src/*`
- engine 测试：`crates/engine/tests/engine_flow.rs`
- eval/bench：`crates/engine/src/eval.rs`、`crates/engine/benches/*`

## 项目定位

memo-brain 是本地 CLI 记忆引擎。

它不是通用数据库、不是通用全文搜索工具、不是 provider 调用包装器，也不是常驻服务。它的产品目标是：把用户明确写入的材料保存为本地真相源，并通过显式维护命令把自然语言材料沉淀成可检索、可整理、可追溯的记忆。

最终核心原则：

- SQLite 是唯一真相源；
- text index、vector index、cache、index state 都是派生服务层；
- provider 是正常智能使用的前置条件，不是可有可无的增强项；
- provider 调用不得进入默认 `recall`；
- 智能能力必须先通过显式维护命令沉淀到 SQLite 和本地派生层；
- 默认查询只读本地数据，稳定快返回；
- 不引入后台、worker、daemon、自动调度或隐式异步队列；
- 慢操作必须由显式 CLI 命令触发；
- 当前已实现能力和最终目标必须在文档里分清楚。

## 用户心智模型

最终产品对用户只暴露少量稳定概念：

- 写入：`remember`
- 查询：`recall`
- 查看单条：`reflect`
- 整理：`dream`
- 查看状态：`state`
- 初始化：`awaken`
- 记忆成熟度：L1 / L2 / L3
- 最近活跃：Working Set
- 显式保护：Pinned
- 智能前提：provider readiness

以下内容是内部实现或遗留实现，不应成为用户需要理解的产品概念：

- `index_jobs`
- `index_state`
- text/vector index 文件
- L3 cache
- session cache / L0
- restore / rebuild / repair queue
- capability mode / maintenance mode
- Core / L4 / 身份核心层

`state` 可以展示内部状态摘要，但普通输出必须落到一个用户动作上：配置 provider、运行 `dream`、或无需处理。不要为每个内部状态新增一个公开概念。

## Workspace 架构

当前 workspace 有三个主要层次。

### 根 crate `memo`

路径：`src/*`

职责：

- CLI entrypoint；
- clap 参数解析；
- 默认 `~/.memo` 配置目录和 `~/.memo/data` 数据目录解析；
- `~/.memo/config.toml`、`~/.memo/providers.toml` 初始化；
- provider 配置解析；
- provider readiness 计算；
- provider runtime 状态读取；
- provider adapter 组装；
- CLI text/json 输出渲染；
- 将命令输入翻译成 `memo-engine` API 调用。

关键文件：

- `src/main.rs`：入口；
- `src/cli/args.rs`：命令定义和参数解析；
- `src/cli/commands.rs`：命令执行；
- `src/cli/output/*`：输出渲染；
- `src/cli/paths.rs`：本地路径；
- `src/config/*`：配置模板、解析和 engine config 构建；
- `src/providers/adapters/*`：把 lmkit provider 适配成 engine trait；
- `src/providers/runtime.rs`：provider 调用失败/恢复记录；
- `src/providers/status.rs`：provider readiness 与 runtime 汇总。

边界：

- 根 crate 可以依赖 `memo-engine` 和 `lmkit`；
- 根 crate 不应承载层级晋升、冲突处理、召回排序、dream 规则等核心记忆逻辑；
- provider 网络细节留在根 crate 和 `lmkit`，不下沉到 engine。

### `crates/engine`

路径：`crates/engine/src/*`

职责：

- SQLite 真相源；
- record 数据模型；
- L1/L2/L3 成熟度层级；
- lifecycle 状态；
- anchored/Pinned 元数据；
- recall 候选召回、融合排序、MMR；
- dream 整理、抽取接入、去重、晋升、冲突失效、冷却；
- Tantivy text index；
- HNSW vector index；
- index state 与 index repair bookkeeping；
- 派生层修复与刷新；
- engine state；
- eval runner；
- benchmark。

关键文件：

- `crates/engine/src/db/schema.rs`：SQLite schema；
- `crates/engine/src/db/write.rs`：写入 episode/entity/fact/edge 和 index jobs；
- `crates/engine/src/db/read.rs`：读取和统计；
- `crates/engine/src/db/search.rs`：exact/alias/text/vector/graph 所需读取；
- `crates/engine/src/db/layers.rs`：layer、archive、invalidate、hit count、anchor；
- `crates/engine/src/db/index_jobs.rs`：index job 入账；
- `crates/engine/src/db/index_state.rs`：index 状态读取和修复账本观测；
- `crates/engine/src/engine/ingest.rs`：remember 和结构化写入；
- `crates/engine/src/engine/recall/*`：recall pipeline；
- `crates/engine/src/engine/dream.rs`：dream pipeline；
- `crates/engine/src/engine/restore.rs`：当前遗留 restore/full restore 实现，最终应删除公开命令语义并把可保留的派生层维护能力迁入 `dream`；
- `crates/engine/src/text_index.rs`：Tantivy；
- `crates/engine/src/vector_index.rs`：HNSW；
- `crates/engine/src/eval.rs`：eval runner；
- `crates/engine/benches/*`：latency/quality bench。

边界：

- engine 不应依赖具体 provider SDK；
- engine 只通过 trait 接收 `EmbeddingProvider`、`ExtractionProvider`、`RerankProvider`；
- engine 不应知道 `providers.toml` 的 provider 配置格式或用户配置目录；
- engine 可以保存 provider 产物，但不应保存 provider secret。

### `crates/lmkit`

路径：`crates/lmkit/src/*`

职责：

- 多 provider AI client；
- chat、embed、rerank、image、audio 等能力；
- OpenAI-compatible、Gemini、Anthropic-compatible、Aliyun、Zhipu 等适配；
- HTTP 错误、重试判断、SSE、类型定义；
- 独立文档和示例。

边界：

- `lmkit` 是可复用库，不应知道 memo-brain 的 SQLite schema、记忆层级、CLI 命令或 dream/recall 语义；
- memo-brain 通过 root crate adapter 使用 `lmkit`，不让 engine 直接耦合 provider client。

## 文件与运行时产物

当前 CLI 默认使用用户 home 下的 `~/.memo` 作为配置目录，而不是当前项目目录下的 `.memo`。默认数据目录是 `~/.memo/data`，也可以通过 `MEMO_DATA_DIR` 或 `[storage].data_dir` 覆盖；相对路径按配置目录解析。

当前主要产物：

- `~/.memo/config.toml`：应用配置；
- `~/.memo/providers.toml`：provider 配置；
- `~/.memo/data/memory.db`：默认 SQLite 真相源；
- `~/.memo/data/text-index/`：默认 Tantivy index；
- `~/.memo/data/vector-index.json`：默认 vector manifest；
- `~/.memo/data/vector-index.hnsw.graph`：默认 HNSW sidecar；
- `~/.memo/data/vector-index.hnsw.data`：默认 HNSW sidecar；
- `~/.memo/data/provider-runtime.json`：provider runtime health，不是 SQLite 表。

最终语义：

- `memory.db` 是唯一真相源；
- text/vector 文件和 sidecar 都是 SQLite 派生材料，可由 `dream` 的显式维护流程修复或刷新；
- `provider-runtime.json` 只记录 provider 调用健康状态，不参与记忆内容；
- 配置模板存在不代表智能能力 ready；
- `awaken` 初始化的是固定用户配置空间，不接受任意 path 参数；
- 文档、测试和安装脚本必须避免把 repo-local `.memo` 写成默认运行目录。

## 公开命令体系

最终公开命令：

- `awaken`
- `remember`
- `recall`
- `reflect`
- `dream`
- `state`

当前已实现命令：

- `awaken`
- `remember`
- `recall`
- `reflect`
- `dream`
- `state`

当前公开命令面已经不包含 `restore`。最终标准继续不新增 `rebuild`，派生层维护统一归入 `dream`。

### awaken

当前实现：

- `src/cli/args.rs` 中 `Awaken` 无 path 参数；
- `src/cli/commands.rs` 使用默认 config dir 和 data dir；
- `config::initialize_app_home` 写入缺失模板；
- 不调用 provider；
- 不做远程连通性检查。

最终标准：

- 初始化配置模板和本地空间；
- 幂等；
- 不覆盖用户配置；
- 不调用 provider；
- 输出必须说明 placeholder key 不等于智能能力 ready。

### remember

当前实现：

- 写入 `episodes`；
- 可解析手工 `--entity type:name:alias1|alias2`；
- 可解析手工 `--fact subject:predicate:object`；
- 默认 episode layer 为 L1；
- `--time` 支持 RFC3339；
- `MemoryEngine::remember` 默认不调用 embedding；
- 手工 entity/fact 会立即写入结构化记录，并标记 episode `structured_at`；
- 写入后刷新 L3 cache；
- 写入后刷新进程内 session cache；
- 写入后持久化 Working Set；
- 写入时 queue text index job；只有已经存在 `vector_json` 的结构化维护路径才会 queue vector index job。

当前问题：

- 当前 recall 输出已稳定展示 `provider_calls=0` 诊断语义；
- 手工结构化输入仍属于高级入口，普通自然语言结构化依赖后续 `dream`。

最终标准：

- 快速写 SQLite 真相源；
- 保存原始 episode；
- 保存手工 entity/fact；
- 不调用 extraction、embedding、rerank；
- 不自动 dream；
- 不自动补语义；
- 写入成功后持久化 Working Set；
- 派生层同步失败只让 `state` 提示运行 `dream`，不让 SQLite 写入失败。

### recall

当前实现：

- CLI 参数：`query`、`-n/--limit`、`--deep`、`--json`；
- 默认 `include_related_records=false`；
- fast 查询可能自动升级 deep；
- 候选来自 L0/session、L3 cache、exact/alias、Tantivy BM25、graph expansion；
- 默认 recall 不调用 query embedding；
- deep recall 不调用 rerank；
- 查询结束后更新 hit count、持久化 Working Set，并更新进程内 session cache。

当前问题：

- `L0` 是进程内 session cache，不适合作为 CLI 产品语义；
- 输出已稳定展示 `provider_calls=0`、`total_candidates` 和 candidate `capabilities` 诊断语义。

最终标准：

- 默认 `provider_calls=0`；
- 默认只读本地 SQLite、text index、已有 vector index、graph relations、L3 cache、Working Set；
- 不调用 extraction；
- 不调用 embedding；
- 不调用 rerank；
- 不自动 dream；
- active L1/L2/L3 都参与；
- L2 是主力结构化来源；
- L1 是证据兜底；
- L3 是稳定和热度加权来源，不能压过更匹配的 L2；
- Working Set 是持久化最近活跃横切视图，只做小幅加权；
- 查询命中后写回 hit count 和 Working Set。

### reflect

当前实现：

- CLI 参数：`id`、`--json`；
- 通过 `engine.reflect(id)` 读取记录；
- 输出单条 memory record；
- `engine.reflect(id)` 会把该记录写入持久化 Working Set。

最终标准：

- 只读 SQLite 真相源记录；
- 可查看 active、archived、invalidated 记录；
- 更新该记录的 Working Set；
- 不改 layer；
- 不改 status；
- 不调用 provider。

### dream

当前实现：

- CLI 参数当前包含 `--full`、`--json`；
- `dream` 调 `engine.dream(DreamTrigger::Manual)`；
- 当前 `dream --full` 调 `dream_full`，最多两 pass；
- `dream` 会在 extraction provider 可用时结构化未处理 episode；
- `dream` 同时执行规则维护：去重、晋升、冲突失效、事实合并、L3 冷却、L3 cache refresh；
- `dream` 会处理 pending/failed `index_jobs` 的常规派生层维护；
- `dream --full` 会从 SQLite truth source 全量刷新 text/vector 派生层；
- `dream` 已开始尊重 Pinned 保护；
- provider 不可用时会在 report 里写 maintenance note。

当前问题：

- `dream` 输出已包含派生层维护统计，但 extraction/embedding provider call 情况还需要继续细化；
- Pinned 保护已覆盖主要自动 archive/invalidate/cooling 路径，CLI 和用户输出命名仍需继续从 anchored 收敛到 Pinned。

最终标准：

- 日常整理与收敛入口；
- 公开命令中，只有 `dream` 允许调用 extraction provider；
- 把未结构化 episode 沉淀成 entity/fact/edge；
- 处理去重、晋升、冲突、冷却；
- 尊重 Pinned；
- 修正常规派生状态；
- 可以调用 embedding，为新结构化或缺失向量的 active records 生成 `vector_json`；
- 不调用 rerank；
- `dream` 是显式整理命令，允许在整理时生成或补齐向量；embedding 不得进入默认 `remember` 或默认 `recall`；
- `dream` 输出必须说明 extraction/embedding provider call 情况。

### state

当前实现：

- 展示 episode/entity/fact/edge count；
- 展示 unstructured L1/L2；
- 展示 structured total；
- 展示 anchored/Pinned record count；
- 展示 layer summary；
- 展示 L3 cache count；
- 展示 text/vector index status；
- 读取默认数据目录下的 `provider-runtime.json`；
- 读取 provider readiness；
- 输出 text/json。

当前问题：

- 普通输出已收敛为 `status`、`message`、`next`；
- JSON 输出已提供 `diagnostics.internal_reasons`；
- anchored 兼容字段仍未完全从对外命名中移除；
- Working Set 统计和展示还可继续细化。

最终标准：

- 显示 provider readiness、runtime health、能力状态；
- 显示结构化状态、派生层状态、Working Set、Pinned；
- 明确提示是否需要 `dream`；
- 不把 provider 缺失说成普通增强缺失。

## 记忆模型

memo-brain 的记忆模型是多轴模型，不是单一 L 轴。

### 成熟度轴

#### L1 Evidence

含义：

- 原始 episode 证据层；
- 尚未结构化或低整理度的材料；
- recall 的证据兜底；
- dream 的主要输入。

当前实现：

- episode 默认写入 L1；
- 手工 entity/fact 会跟随 episode layer；
- duplicated L1 episode 会被 dream 处理；
- L1 episode hit count 达到条件可晋升 L2；
- L1 facts/entities 可按支持度晋升 L2。

最终标准：

- L1 不等于短期 session；
- L1 是 SQLite 中可追溯证据；
- L1 不因结构化而删除原文；
- 大量临时信息可以留在 L1，但是否下沉由 dream 判断。

#### L2 Structured

含义：

- 结构化主工作层；
- entity/fact/edge 的主要查询来源；
- 默认 recall 的主力。

当前实现：

- `entity`、`fact`、`edge` 都有 layer；
- dream 会将支持度足够的 episode/entity/fact/edge 推到 L2；
- `MemoryLayer::L2.boost()` 当前为 `0.12`；
- L2 active 记录参与 text/vector/graph recall。

最终标准：

- L2 是结构化召回主力；
- L2 强匹配必须优先于弱相关 L3；
- graph expansion 以 L2/L3 structured record 为主要基础。

#### L3 Stable

含义：

- 长期稳定热点层；
- 代表高支持度、高命中或跨时间验证；
- 提供稳定性和热度加权。

当前实现：

- `MemoryLayer::L3.boost()` 当前为 `0.25`；
- `l3_cache_limit` 默认 256；
- engine open 后刷新 L3 cache；
- dream 通过 hit count、支持 scope 和时间跨度晋升 L3；
- stale L3 可冷却回 L2；
- L3 cache 按 updated/activity 排序后截断；
- recall 中 L3 cache 可直接给候选。

当前关键规则：

- entity 晋升 L3：L2 entity 至少 3 个 support scope，且最早和最晚支持跨度不少于 1 天；
- fact merge 晋升 L3：同 subject/predicate/object 至少 3 个 distinct support scope，且跨度不少于 1 天；
- 通用 L3 晋升：L2 episode/entity/fact hit_count >= 2；
- L3 冷却：超过 30 天 stale，episode hit_count <= 2 可冷却，entity/fact hit_count <= 1 可冷却，edge 不参与冷却；
- `last_l3_promoted_at` 用于避免刚冷却又被重复晋升。

最终标准：

- L3 不能靠层级 boost 压过更匹配的 L2；
- Pinned L3 不被自动冷却；
- L3 是成熟度层，不是最深层人格记忆，不引入 L4。

### 活跃度轴：Working Set

含义：

- 最近活跃横切视图；
- 不是 L0；
- 不是进程内 session；
- 横切 L1/L2/L3。

当前实现：

- `memory_layers` 已有 `working_set_at`；
- remember 写入 episode、手工 entity/fact 时会记录 Working Set；
- recall 对最终命中的 active records 写回 Working Set；
- reflect 查看记录时会写回 Working Set；
- dream 晋升、归档或失效相关记录时会更新对应活跃视图；
- 进程内 `SessionCache` 仍存在，用于补充短期最近上下文，但不再是 Working Set 的唯一来源。

最终标准：

- 在 SQLite 中持久化，例如 `memory_layers.working_set_at`；
- 进入来源包括 remember、recall 命中、reflect 查看、dream 生成或更新结构化记录；
- 默认窗口建议 7 天；
- 只做小幅最近活跃加权；
- 不能压过明确匹配；
- `state` 必须显示 Working Set 状态。

### 保护轴：Pinned

含义：

- 长期保护横切标记；
- 不是 L4；
- 不代表更高成熟度；
- 横切 L1/L2/L3。

当前实现：

- `memory_layers` 已有 `pinned_at`、`pinned_reason`，并保留 `anchored_at` 作为兼容字段；
- schema v4 会把旧 `anchored_at` 兼容迁移到 `pinned_at`；
- DB API 已有 `pin_record`、`unpin_record`、`pinned_record_count`，旧 `anchor_record`/`unanchor_record` 仍作为兼容桥；
- engine API 已有 `pin`、`unpin`；
- dream 已在主要 cooling、archive、invalidate、merge 路径尊重 Pinned；
- `state` 同时保留 anchored 兼容计数和 Pinned 计数。

最终标准：

- `anchored_at` 收敛为 `pinned_at`；
- 增加 `pinned_reason`；
- 用户心智统一为 Pinned；
- Pinned 默认由用户显式设置；
- dream 不得自动冷却、归档、失效、覆盖 Pinned record；
- recall 可轻量加权 Pinned，但不能让不匹配内容强行排前；
- `state` 显示 Pinned count 和状态。

不要引入 L4、Core 或身份核心层。Pinned 的含义只限于“这条记录受到显式保护”，不能扩展成新的记忆层。

### 生命周期轴

当前实现：

- 业务表使用 `archived_at`、`invalidated_at`；
- `memory_layers.status` 使用 `active`、`archived`、`invalidated`；
- archive/invalidate 会同步业务表和 `memory_layers`；
- archive/invalidate 会 queue text/vector delete jobs；
- 默认 active 查询过滤 archived/invalidated。

最终标准：

- `active` 参与默认 recall；
- `archived` 不参与默认 recall，但保留追溯；
- `invalidated` 不参与默认 recall，但保留冲突解释；
- Archived/Invalidated 不是 L 维度，不是 L4。

## SQLite 真相源

当前 schema version 为 `3`。

SQLite 表：

- `episodes`
- `entities`
- `entity_aliases`
- `mentions`
- `facts`
- `edges`
- `memory_layers`
- `index_state`
- `index_jobs`

当前初始化会 `DROP TABLE IF EXISTS dream_jobs`，说明旧 dream jobs 概念已经被移除。最终设计也不引入后台 dream job。

### `episodes`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS episodes (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  normalized_content TEXT NOT NULL,
  layer TEXT NOT NULL,
  confidence REAL NOT NULL,
  source_episode_id TEXT NULL,
  session_id TEXT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  archived_at INTEGER NULL,
  invalidated_at INTEGER NULL,
  hit_count INTEGER NOT NULL DEFAULT 0,
  structured_at INTEGER NULL,
  vector_json TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_episodes_normalized
  ON episodes(normalized_content);

CREATE INDEX IF NOT EXISTS idx_episodes_layer
  ON episodes(layer);
```

字段语义：

- `id`：UUID；
- `content`：用户原始文本；
- `normalized_content`：归一化文本，用于 exact/duplicate；
- `layer`：当前业务表 layer 镜像；
- `confidence`：写入置信度；
- `source_episode_id`：派生 episode 的来源；
- `session_id`：支持度 scope，可用于跨 session 判断；
- `created_at`：记录时间；
- `updated_at`：更新时间；
- `last_seen_at`：最近被观察/命中时间；
- `archived_at`：归档时间；
- `invalidated_at`：失效时间；
- `hit_count`：召回命中累计；
- `structured_at`：已被结构化处理时间；
- `vector_json`：已有 embedding 向量 JSON。

当前读写：

- `insert_episode` 写入 episode 和 `memory_layers`；
- 写入时 queue text index upsert；
- 有 vector 时 queue vector index upsert；
- duplicate L1 grouping 依赖 `normalized_content`；
- unstructured 统计依赖 `structured_at IS NULL`。

最终标准：

- `content` 是证据，不因 dream 被覆盖；
- `structured_at` 只表示处理过，不表示提取质量必然高；
- `vector_json` 是本地派生材料，默认 recall 不现场补；
- `layer` 最终可保留为查询优化镜像，权威应收敛到 `memory_layers`。

### `entities`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS entities (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  canonical_name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  confidence REAL NOT NULL,
  source_episode_id TEXT NULL,
  layer TEXT NOT NULL DEFAULT 'L1',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  archived_at INTEGER NULL,
  invalidated_at INTEGER NULL,
  hit_count INTEGER NOT NULL DEFAULT 0,
  vector_json TEXT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_entities_normalized
  ON entities(normalized_name);
```

字段语义：

- `entity_type`：实体类型；
- `canonical_name`：规范名；
- `normalized_name`：规范名归一化，用于去重和 exact；
- `source_episode_id`：首次来源 episode；
- `last_seen_at`：最近出现或命中；
- `vector_json`：实体名 embedding。

当前读写：

- `upsert_entity` 先按 normalized name 或 alias 找 active entity；
- 既有 entity 会更新 confidence、entity_type、updated/last_seen；
- 新 entity 写入 `memory_layers`；
- aliases 写入 `entity_aliases`；
- entity 支持度可来自 mentions 和 facts；
- entity 可晋升 L2/L3；
- entity 参与 alias recall、text index、vector index、graph seeds。

最终标准：

- entity 是 L2 结构化主力；
- alias 命中后必须回 SQLite 读取 active entity；
- entity 的 Pinned 和 Working Set 通过 `memory_layers` 表示。

### `entity_aliases`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS entity_aliases (
  id TEXT PRIMARY KEY,
  entity_id TEXT NOT NULL,
  alias TEXT NOT NULL,
  normalized_alias TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(entity_id, normalized_alias)
);

CREATE INDEX IF NOT EXISTS idx_entity_aliases_normalized
  ON entity_aliases(normalized_alias);
```

字段语义：

- `entity_id`：所属 entity；
- `alias`：原始别名；
- `normalized_alias`：归一化别名；
- `created_at`：创建时间。

当前读写：

- `upsert_entity` 插入 alias；
- `search_exact_alias` 用 normalized alias 找 entity；
- `find_active_entity_id_by_reference` 解析 fact subject/object entity。

最终标准：

- alias 是结构化检索入口；
- alias 不独立成为 Working Set；
- alias 命中仍受 entity active status 约束。

### `mentions`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS mentions (
  id TEXT PRIMARY KEY,
  episode_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  role TEXT NOT NULL,
  confidence REAL NOT NULL,
  created_at INTEGER NOT NULL
);
```

字段语义：

- `episode_id`：提及发生的 episode；
- `entity_id`：被提及 entity；
- `role`：当前主要为 `mentioned`；
- `confidence`：提及置信度。

当前读写：

- 手工/抽取 entity 写入 mention；
- fact subject/object resolve 后确保 mention；
- dream 会从 active facts backfill mentions；
- entity 支持度统计依赖 mentions 和 facts。

最终标准：

- mentions 是 evidence 与 structured entity 的桥；
- 不作为独立 recall 结果；
- 可用于支持度、上下文、graph seed。

### `facts`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS facts (
  id TEXT PRIMARY KEY,
  subject_entity_id TEXT NULL,
  subject_text TEXT NOT NULL,
  predicate TEXT NOT NULL,
  object_entity_id TEXT NULL,
  object_text TEXT NOT NULL,
  confidence REAL NOT NULL,
  source_episode_id TEXT NULL,
  layer TEXT NOT NULL,
  valid_from INTEGER NULL,
  valid_to INTEGER NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER NULL,
  invalidated_at INTEGER NULL,
  hit_count INTEGER NOT NULL DEFAULT 0,
  vector_json TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_facts_layer
  ON facts(layer);
```

字段语义：

- `subject_entity_id` / `object_entity_id`：实体引用；
- `subject_text` / `object_text`：文本保底；
- `predicate`：关系谓词；
- `valid_from` / `valid_to`：事实有效时间窗口；
- `source_episode_id`：证据来源；
- `vector_json`：fact 文本 embedding。

当前读写：

- `insert_fact` 写入 fact 和 `memory_layers`；
- fact 写入时 queue text/vector upsert；
- fact 会触发 edge 插入；
- conflict grouping 使用 normalized subject + predicate；
- supported fact merge 使用 normalized subject + predicate + object；
- archive/invalidate fact 会设置 `valid_to`；
- fact 是 recall text/vector/graph 的主要 structured record。

最终标准：

- fact 是 L2 结构化 recall 主力；
- invalidated fact 保留用于解释；
- Pinned fact 不应被 dream 自动 invalidated/archive；
- L3 fact 只做稳定性和热度加权，不能压过强匹配 L2。

### `edges`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS edges (
  id TEXT PRIMARY KEY,
  subject_entity_id TEXT NOT NULL,
  predicate TEXT NOT NULL,
  object_entity_id TEXT NOT NULL,
  weight REAL NOT NULL,
  source_episode_id TEXT NULL,
  layer TEXT NOT NULL,
  valid_from INTEGER NULL,
  valid_to INTEGER NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER NULL,
  invalidated_at INTEGER NULL,
  hit_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_edges_subject
  ON edges(subject_entity_id);

CREATE INDEX IF NOT EXISTS idx_edges_object
  ON edges(object_entity_id);
```

字段语义：

- `subject_entity_id` / `object_entity_id`：实体图端点；
- `predicate`：边关系；
- `weight`：边权重，当前来自 fact confidence；
- `source_episode_id`：证据来源；
- `valid_from` / `valid_to`：边有效窗口。

当前读写：

- fact 写入后插入 edge；
- graph expansion 从 seed entity 扩展 edges/facts；
- archive/invalidate fact 时会匹配并处理 edge；
- edges 不进入 text/vector document load；
- edge L3 不参与 stale cooldown。

最终标准：

- edge 是 graph relation，不一定作为默认直接输出；
- `include_related_records` 控制是否扩展输出 graph records；
- graph expansion 不能因为 `deep` 或 `limit` 隐式改变用户可见语义。

### `memory_layers`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS memory_layers (
  memory_id TEXT NOT NULL,
  memory_kind TEXT NOT NULL,
  layer TEXT NOT NULL,
  status TEXT NOT NULL,
  last_promoted_at INTEGER NULL,
  last_l3_promoted_at INTEGER NULL,
  anchored_at INTEGER NULL,
  working_set_at INTEGER NULL,
  pinned_at INTEGER NULL,
  pinned_reason TEXT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(memory_id, memory_kind)
);
```

字段语义：

- `memory_id`：业务记录 id；
- `memory_kind`：`episode`、`entity`、`fact`、`edge`；
- `layer`：L1/L2/L3；
- `status`：active/archived/invalidated；
- `last_promoted_at`：最近晋升时间；
- `last_l3_promoted_at`：最近进入 L3 时间；
- `anchored_at`：旧锚定兼容字段；
- `working_set_at`：最近活跃横切视图；
- `pinned_at`：显式保护时间；
- `pinned_reason`：显式保护原因；
- `created_at` / `updated_at`：元数据时间。

当前读写：

- 每个业务 record 写入时同时插入 memory_layers；
- `update_layer` 同步业务表 layer 和 memory_layers layer；
- L3 晋升写 `last_l3_promoted_at`；
- archive/invalidate 更新 `status`；
- `mark_working_set` / `mark_working_set_records` 更新 `working_set_at`；
- `pin_record` / `unpin_record` 更新 `pinned_at`、`pinned_reason`，并同步旧 `anchored_at` 兼容字段；
- `anchor_record` / `unanchor_record` 作为兼容桥调用 pin/unpin；
- `layer_summary` 从该表汇总。

目标结构继续保持：

```sql
memory_layers(
  memory_id TEXT NOT NULL,
  memory_kind TEXT NOT NULL,
  layer TEXT NOT NULL,
  status TEXT NOT NULL,
  last_promoted_at INTEGER NULL,
  last_l3_promoted_at INTEGER NULL,
  working_set_at INTEGER NULL,
  pinned_at INTEGER NULL,
  pinned_reason TEXT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(memory_id, memory_kind)
)
```

迁移要求：

- 旧 `anchored_at` 已通过 schema v4 迁移或兼容读为 `pinned_at`；
- `working_set_at` 已加入 schema v4；
- `pinned_reason` 已加入 schema v4；
- 业务表 layer 继续作为查询优化镜像，但权威语义逐步收敛到 `memory_layers`。

最终读写约束：

- `remember` 写入 episode、手工 entity/fact 后，把相关 record 的 `working_set_at` 更新为写入时间；
- `recall` 只对最终命中的 active records 更新 `working_set_at`，不对被过滤、去重或仅参与排序的候选写入；
- `reflect` 查看任意记录时更新该记录 `working_set_at`，但不改变 layer/status；
- `dream` 只对新生成、被晋升、被合并保留或被显式更新的 active records 更新 `working_set_at`；
- 派生层维护不更新 `working_set_at`、`pinned_at` 或 `pinned_reason`；
- `pinned_at` 非空时，自动 cooling/archive/invalidate/merge 只能跳过该记录并在 report 中计数，不能静默覆盖；
- `pinned_reason` 只保存用户或上层调用给出的简短原因，不保存 provider 原始响应。

### `index_state`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS index_state (
  index_name TEXT PRIMARY KEY,
  doc_count INTEGER NOT NULL,
  status TEXT NOT NULL,
  detail TEXT NULL,
  last_rebuilt_at INTEGER NULL
);
```

字段语义：

- `index_name`：`text` 或 `vector`；
- `doc_count`：上次记录的文档数；
- `status`：unknown/ready/pending/failed；
- `detail`：状态说明；
- `last_rebuilt_at`：当前字段名，表示最近一次派生层达到 ready 的时间；最终可保留字段名但对外不暴露 rebuild 命令语义。

当前读写：

- queue index job 会把 index_state 标记 pending；
- 当前遗留 restore 成功会标记 ready；
- 当前遗留 restore 失败会标记 failed；
- `index_status` 会合并 `index_jobs` 的 pending/failed 状态。

最终标准：

- index_state 描述派生层可信度；
- 派生层不可信时普通输出仍只提示运行 `dream`，内部 diagnostics 可以记录具体原因；
- detail 文案应改成 “dream maintenance required” 一类维护口径，不出现 restore/rebuild 命令建议。

### `index_jobs`

当前 DDL：

```sql
CREATE TABLE IF NOT EXISTS index_jobs (
  id TEXT PRIMARY KEY,
  index_name TEXT NOT NULL,
  memory_kind TEXT NOT NULL,
  memory_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  status TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(index_name, memory_kind, memory_id)
);
```

字段语义：

- `index_name`：`text` 或 `vector`；
- `memory_kind`：记录类型；
- `memory_id`：记录 id；
- `operation`：upsert/delete；
- `status`：pending/failed；
- `attempts`：失败次数；
- `last_error`：最后错误；
- `created_at` / `updated_at`：账本时间。

当前读写：

- episode/entity/fact 写入或 layer 更新 queue text upsert；
- 有 vector 的 episode/entity/fact queue vector upsert；
- archive/invalidate queue text/vector delete；
- 当前遗留 `restore` 增量消费 outstanding jobs；
- 当前遗留 `restore_full` 成功后清理对应 index jobs。

最终语义：

- `index_jobs` 可以保留；
- 它是“派生状态未同步/修复账本”；
- 不是后台任务队列；
- 不是用户概念；
- 正常情况下应为空；
- `dream` 可处理常规修复；
- `dream` 维护成功后以 SQLite 为准清理相关账本。

最终读写约束：

- SQLite 业务写入成功后，派生层 upsert/delete 失败不得回滚业务写入；
- 写入派生层失败时只记录或保留 `index_jobs`，并把对应 `index_state` 标记为 pending/failed；
- `dream` 的常规修复只能读取 pending/failed `index_jobs`，按 job 指向的 SQLite active truth record 生成 upsert/delete update；
- 如果 job 指向的 record 已不存在或已退出 active，修复应转成 delete update 或清理账本，不能重新创建业务记录；
- 当前全量刷新路径可以忽略 `index_jobs` 范围，按 SQLite active episodes/entities/facts 刷新派生文档；最终不要求用户理解或直接选择该路径；
- `dream` 维护失败时保留诊断信息并把 `index_state` 标记 failed。

## Provider 与配置

### 配置文件

当前配置文件：

- `~/.memo/config.toml`
- `~/.memo/providers.toml`

当前 `config.toml` 覆盖：

- storage/data dir；
- engine 参数；
- embedding provider ref；
- extraction provider ref；
- rerank provider ref；
- provider retry 参数；
- extraction cleanup 参数。

当前 `providers.toml` 覆盖：

- provider api key；
- provider service base url；
- model；
- dimension；
- timeout；
- max concurrent。

当前模板来源：

- `templates/config.toml`；
- `templates/providers.toml`；
- `src/config/templates.rs` 通过 `include_str!` 将模板编进 CLI；
- `awaken` 只在文件缺失时写模板，不覆盖已有文件。

当前实现限制：

- parser 是窄 TOML parser；
- 不能默认假设支持完整 TOML；
- 新增配置键必须更新 parser 和测试；
- placeholder key 会被 readiness 识别。

### Provider trait

engine trait：

```rust
pub trait EmbeddingProvider: Send + Sync {
  fn dimension(&self) -> usize;
  fn embed_text(&self, text: &str) -> Result<Vec<f32>>;
}

pub trait ExtractionProvider: Send + Sync {
  fn extract(&self, text: &str) -> Result<ExtractionResult>;
}

pub trait RerankProvider: Send + Sync {
  fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<RerankScore>>;
}
```

`Send + Sync` 是 engine 对 provider adapter 的真实接口约束。根 crate 当前以 `Arc<dyn ...>` 组装 provider，并通过 retry wrapper 记录 runtime health，因此最终设计不能把 provider 视为普通非线程安全对象。

当前 extraction result：

- entities：`entity_type`、`name`、`aliases`、`confidence`；
- facts：`subject`、`predicate`、`object`、`confidence`。

### Provider runtime

当前 provider runtime 存储在数据目录下的 `provider-runtime.json`，默认路径是 `~/.memo/data/provider-runtime.json`，不是 SQLite 表。

记录内容：

- capability；
- provider ref；
- status：ok/degraded；
- consecutive failures；
- last error；
- updated at。

当前 retry wrapper：

- embedding、extraction、rerank 都有 retrying wrapper；
- retryable 错误根据 `lmkit::Error::is_retryable` 判断；
- 成功记录 ok；
- 最终失败记录 degraded。

最终标准：

- provider runtime 只描述调用健康；
- 不能用 runtime 替代 readiness；
- state 同时展示配置 readiness 和 runtime degradation。

### Readiness 与能力状态

当前 readiness 状态：

- `not_configured`
- `placeholder_key`
- `configured`
- `degraded`
- `ok`

最终能力状态：

- `not_ready`：必要 provider 未配置或 placeholder；
- `structure_ready`：extraction 静态配置可用，`dream` 具备自动结构化前置条件；
- `semantic_ready`：结构化数据和本地向量派生层可用，默认 recall 可做本地语义召回。

关键原则：

- extraction provider 是正常智能使用必需；
- placeholder key 等同未配置；
- provider 缺失时只能承诺原文记录和状态查看；
- 不能宣称智能记忆可用；
- `configured` 只是静态配置通过，不代表远程 provider 已经连通；
- `structure_ready` 不是“provider 已远程验证成功”，只表示 `dream` 可以尝试自动结构化；
- `ok` 来自最近一次成功调用，只能作为 runtime health，不改变默认 `recall` 的 provider 调用边界；
- `degraded` 表示最近调用失败，不能自动降级成“无 provider 也智能可用”。

命名约束：

- 对外只说“能力状态”，不说 capability mode；
- 对外只说“需要 dream”，不说 maintenance mode；
- 对外不说 repair queue，只说“派生层未同步”或“派生层不可信”；
- 对外不说 L0/session，只说 Working Set。

## Provider 调用边界

### extraction provider

当前实现：

- 只在 dream 的 `structure_pending_episodes` 路径调用；
- `remember` 默认不调用 extraction；
- extraction 输出经过 merge 后写入 entity/fact/edge。

最终标准：

- 只允许 `dream` 调用；
- 无 extraction 时 `dream` 明确报告无法自动结构化；
- `state` 显示 `not_ready`，并在 detail 中说明 extraction 未就绪。

### embedding provider

当前实现：

- `remember` 默认不调用 embedding；
- `ingest_episode_structure` 在 `dream` 的 provider 结构化路径中会为 entity/fact 尝试 embedding；
- `recall` 不调用 query embedding；
- 当前遗留 `restore_full` 不调用 embedding，只读取已有 `vector_json`。

最终标准：

- 默认 `remember` 不调用；
- 默认 `recall` 不调用；
- `dream` 可以调用，用于把 SQLite 中的 active episode/entity/fact materialize 成本地 `vector_json`；
- 向量必须提前沉淀为本地材料；
- 缺失向量补齐属于 `dream` 的慢维护职责，必须在输出中标记 provider call；
- `semantic_ready` 只能基于 SQLite 中已经存在的 `vector_json` 和 ready vector index 判断；即使 embedding provider 已配置，默认 `recall` 也不能现场补 query 或 document embedding。

### rerank provider

当前实现：

- engine config 仍可装配 rerank provider；
- 默认 recall 和 deep recall 都不调用 rerank；
- provider runtime/config 层仍保留 rerank adapter 与健康记录能力。

最终标准：

- 不进入默认 recall；
- 最终产品默认不暴露 rerank 查询选项；如保留，只作为离线评估能力；
- 输出必须标记 provider call；
- 不影响默认查询时延。

## 派生层模型

### Text Index

当前实现：

- 使用 Tantivy；
- 默认 index directory 为 `~/.memo/data/text-index/`；
- schema 字段：`id`、`kind`、`layer`、`body`；
- 支持全量刷新、apply updates、search；
- query parse 失败后 sanitize punctuation 再 parse；
- 文档来源：active episodes、entities、facts；
- entity body 包含 canonical name 和 aliases；
- fact body 是 subject predicate object；
- edges 不进入 text index。

最终标准：

- text index 是派生层；
- 命中后必须回 SQLite 读取 active truth record；
- `dream` 可按需要刷新；
- repair bookkeeping 不等于后台任务。

### Vector Index

当前实现：

- 使用 `hnsw_rs`；
- distance 为 cosine；
- manifest 默认为 `~/.memo/data/vector-index.json`；
- sidecar 默认为 `~/.memo/data/vector-index.hnsw.graph` 和 `~/.memo/data/vector-index.hnsw.data`；
- records 以 `kind:id` 为 key；
- 全量刷新或 apply_updates 后重写 HNSW 并 persist；
- search 返回 `(1.0 - distance)` 分数；
- dimension 来自 engine config，embedding provider 存在时由 provider.dimension 覆盖；
- 文档来源：active episodes/entities/facts 且 `vector_json IS NOT NULL`；
- edges 不进入 vector index。

最终标准：

- vector index 是派生层；
- 只使用 SQLite 已有 vector；
- 默认 recall 不现场 embed query；
- vector sidecar 不是真相源。

### L3 Cache

当前实现：

- engine 内部 `HashMap`；
- open/remember/dream 后 refresh；
- 默认 `l3_cache_limit=256`；
- `load_l3_records` 只取 active L3；
- recall 中 L3 cache 如果 text contains normalized query，则给候选。

最终标准：

- L3 cache 是 engine 内部优化；
- 不是 L3 本身；
- 不作为 CLI 单独能力宣传；
- 权威仍在 SQLite。

### Session Cache

当前实现：

- `SessionCache` 保存 recent aliases、recent topics、active subjects、recent memory ids；
- recall reason 中仍有 `L0`；
- L0 exact alias match 分数 3.5；
- WorkingSet boost 当前来自 session cache。

最终标准：

- session cache 只允许作为长生命周期 engine 实例内部优化；
- 不作为 CLI L0 能力；
- 不写进产品分层；
- CLI 跨命令活跃视图必须由 SQLite Working Set 实现；
- 最终输出中不再出现 L0/session 作为用户概念。

## Remember 写入流程

当前流程：

```text
CLI parse content/time/entity/fact
  -> build EpisodeInput(layer=L1)
  -> MemoryEngine::remember
  -> embed_if_available(content)
  -> db.insert_episode
  -> ingest manual entities/facts
  -> entity/fact/edge writes
  -> mark episode structured if manual structure exists
  -> queue index jobs
  -> refresh L3 cache
  -> refresh process-local session cache
  -> return episode id
```

当前算法细节：

- entity upsert 通过 normalized name 或 alias 解析 existing active entity；
- unknown entity_type 可被更具体 entity_type 更新；
- fact subject/object 会解析或创建 entity；
- fact 写入后自动写 edge；
- manual 和 provider extraction 的 merge 以 normalized key 去重；
- embedding 失败会 warn 并继续。

最终流程：

```text
CLI parse
  -> write SQLite truth source
  -> write manual structure only
  -> mark derived state pending/repair if needed
  -> persist Working Set
  -> return id
```

最终禁止：

- 不现场 extraction；
- 不现场 embedding；
- 不现场 rerank；
- 不自动 dream。

## Recall 算法模型

### 当前候选收集

当前 `execute_query` 的候选来源顺序：

- L0/session exact alias；
- L3 cache contains normalized query；
- SQLite exact/alias；
- Tantivy BM25；
- vector index；
- graph expansion。

候选初始分：

- L0：3.5；
- L3：2.4；
- exact/alias：3.0；
- BM25：`0.4 + hit.score * 0.15`；
- vector：`hit.score * 1.2`；
- graph hop：`0.35 / hops`。

deep 参数影响：

- text limit：deep 为 `limit * 12`，非 deep 为 `limit * 6`；
- graph limit：deep 为 `limit * 8`，非 deep 为 `limit * 4`；
- graph hops：deep 为 2，非 deep 为 1；
- deep 不现场调用 rerank；
- deep 后执行 query coverage filtering。

auto deep：

- 初次非 deep 查询如果结果空，会 deep；
- 如果首条没有 decisive reason 且单结果弱或前两名分差小，会 deep；
- decisive reason 当前包括 L0、L3、Exact、Alias。

### 当前 score shaping

当前加权：

- recency boost：`exp(-(age_days / 30)) * 0.18`；
- working set boost：recent memory id 最多 0.22，active subject 最多 0.30；
- layer boost：L1 0.0，L2 0.12，L3 0.25；
- L3 gated boost：subject mismatch 或 query coverage < 0.25 时 L3 boost 为 0；
- hit frequency boost：`ln(hit_count + 1) * 0.05`；
- answer shape boost：current location 类 query 对 `lives_in` fact 加 0.35，对相关 episode 加 0.20；
- subject coverage boost：命中 query subject 每个加 0.35，否则减 0.18；
- subject mismatch 会添加 reason，但不是硬过滤。

当前词法处理：

- 只保留 ascii alphanumeric token；
- token 长度小于 3 或 stopword 会被过滤；
- 有少量英文词形归一，例如 lives/lived/living -> live；
- subject tokens 依赖首字母大写 token，偏英文。

### 当前 rerank

当前逻辑：

- recall pipeline 不调用 rerank；
- rerank provider 仍保留在配置、adapter 和 runtime health 层；
- 如后续用于 eval，应保持离线显式入口，不进入默认 recall。

最终标准：

- 默认 recall 不调用 rerank；
- 如保留 rerank，只能用于离线评估，不能进入默认 recall。

### 当前 filtering、dedupe、MMR

当前过滤：

- deep 时执行 query coverage filtering；
- query token 数少于 2 或候选少于 2 不过滤；
- query token <= 3 时最低 coverage 0.75；
- query token > 3 时最低 coverage 0.60；
- 若过滤后为空，则保留原候选。

当前 dedupe：

- 默认按 source_key 去重；
- source_key 是 `source_episode_id` 或自身 id；
- include_related_records=true 时保留 graph fact/edge 的 source 维度。

当前 small limit tail：

- 如果 `include_related_records=false` 且 limit <= 5，会截断弱尾部；
- top score >= 1.8 时只保留 top_score - 0.85 以内候选。

当前 MMR：

- score = `0.7 * candidate.score - 0.3 * novelty_penalty`；
- novelty_penalty 是与已选结果 text token Jaccard similarity 的最大值；
- 用于减少重复来源附近的相似结果。

### Recall 写回

当前写回：

- `increment_hit_counts` 更新业务表 hit count；
- episode/entity 更新 `last_seen_at`；
- fact/edge 更新 `updated_at`；
- 如果命中 structured record，会同步 source episode hit；
- 更新 process-local session cache。

最终写回：

- hit count；
- `memory_layers.working_set_at`；
- 不写 layer promotion；
- 不写 status；
- 不写 provider runtime；
- 不做 extraction。

### 最终 recall 输出能力位

最终输出必须说明实际能力，例如：

```text
text=true vector=true l1=true l2=true l3=true working_set=true provider_calls=0
```

如果 vector index 不可用，应明确 `vector=false`，但仍不现场调用 provider。

输出约束：

- `provider_calls` 是本次命令实际调用次数，不是配置状态；
- 默认 `recall` 中 `provider_calls` 必须恒为 0；
- `total_candidates` 是去重后的 pre-selection candidate pool，不是底层 raw hits；
- `capabilities` 是候选池来源能力，不等于最终结果的 `reasons`；
- `vector=true` 只表示本地 vector index 参与过查询，不表示本次调用了 embedding provider；
- L1/L2/L3/Working Set 能力位应来自本次查询实际启用的本地来源；
- `working_set=true` 只表示 Working Set 作为本地上下文候选或加权来源参与，不代表 text/vector index ready；
- 如果 query 因派生层损坏跳过 text/vector，应同时输出 next action：运行 `dream`。

## Dream 算法模型

### 当前主流程

当前 `run_dream`：

```text
collect unstructured L1/L2 counts
  -> duplicate L1 episode groups
  -> promote primary duplicate cluster to L2
  -> archive duplicate cluster
  -> structure pending episodes via extraction provider
  -> promote eligible episodes to L2
  -> promote supported entities to L2
  -> promote supported fact clusters to L2
  -> backfill mentions from active facts
  -> invalidate conflicting facts
  -> merge supported fact clusters and promote winner to L3
  -> promote supported entities to L3
  -> promote hit-based episode/entity/fact to L3
  -> cool stale L3 records
  -> refresh L3 cache
```

`dream_full` 当前最多执行 2 pass，并合并 report。只有当上一 pass 有结构化、晋升、冷却、归档或失效变化时才继续。

### Duplicate L1 episode grouping

当前规则：

- 按 `normalized_content` 聚合 L1 active episodes；
- 每组保留最早 created 的 primary；
- primary cluster 晋升 L2；
- duplicate episode 相关 fact/edge archive；
- duplicate episode archive；
- 写入 archived count。

最终标准：

- duplicate 处理是整理，不是删除；
- 原始证据可追溯；
- Pinned duplicate 不能被自动 archive。

### Structure pending episodes

当前规则：

- 如果无 extraction provider，写 maintenance note 并返回；
- load unstructured L1/L2 episodes；
- 对每条调用 extraction provider；
- extraction result 与 manual structure merge；
- 写入 entity/fact/edge；
- mark episode structured；
- provider error 计入 `extraction_failures`，继续后续规则维护；
- extraction 返回 None 时停止结构化。

当前问题：

- 结构化写入仍可能调用 embedding；
- extraction cleanup/prompt/normalize 体系在 root adapter 里，engine 只看 trait result。

最终标准：

- dream 是唯一默认 extraction 入口；
- extraction 失败和 provider 未配置必须在 report/state 中清楚表达；
- 不应把规则维护完成伪装成自动结构化完成。

### Entity 支持度

当前 L2 规则：

- L1 active entity；
- mention 或 fact 支持 scope 至少 2；
- support scope 是 `COALESCE(session_id, episode_id)`。

当前 L3 规则：

- L2 active entity；
- support scope 至少 3；
- 最早和最晚 support created_at 跨度至少 1 天；
- 如果刚发生 stale L3 冷却，避免重复晋升。

最终标准：

- support scope 是防止同一 session 重复刷支持度；
- session_id 是 evidence scope，不是 L0/session cache。

### Fact 支持度与合并

当前 L2 规则：

- L1 active facts；
- subject/predicate/object normalized 后同组；
- distinct support scope 至少 2；
- 同步匹配 L1 edge 晋升 L2。

当前 L3 merge：

- L2/L3 active facts；
- subject/predicate/object normalized 后同组；
- distinct source scope 至少 3；
- 最早和最晚 scope 跨度至少 1 天；
- winner 按 layer boost、hit_count、confidence、updated_at、created_at 比较；
- winner 晋升 L3；
- 其他同组 facts archive；
- 相关 edges archive。

最终标准：

- fact merge 是沉淀稳定事实；
- 被合并事实不直接删除；
- Pinned fact 不被自动 archive。

### Conflict invalidation

当前规则：

- active L1/L2/L3 facts；
- 按 normalized subject + predicate 分组；
- 如果 object 有多个值，形成冲突；
- winner 按 confidence、updated_at、created_at、hit_count、layer boost 比较；
- 非 winner facts invalidated；
- 相关 matching edges invalidated；
- 如果某 source episode 已无 active fact，则相关 entity/edge 和 episode 也 invalidated。

最终标准：

- conflict invalidation 保留证据；
- Pinned record 参与冲突时不能被自动 invalidated；
- report 应说明 pinned skipped 或需要人工处理。

### L3 cooling

当前规则：

- stale threshold 30 天；
- episode hit_count <= 2 且 activity_at stale 可冷却；
- entity/fact hit_count <= 1 且 activity_at stale 可冷却；
- edge 不冷却；
- 冷却是 update layer L3 -> L2。

最终标准：

- cooling 是活跃度变化，不是遗忘；
- Pinned L3 不冷却；
- 冷却不能删除原始证据。

### Dream report

当前字段：

- trigger；
- passes_run；
- unstructured_l1；
- unstructured_l2；
- maintenance_notes；
- structured_episodes；
- structured_entities；
- structured_facts；
- extraction_failures；
- promoted_to_l2；
- promoted_to_l3；
- downgraded_records；
- archived_records；
- invalidated_records。

最终应补充：

- pinned_skipped；
- derived_repairs；
- provider capability summary；
- next action hint。

最终报告语义：

- `structured_episodes` 只统计 extraction 成功沉淀的 episode；
- `promoted_to_l2` / `promoted_to_l3` 只统计 layer 变化；
- `archived_records` / `invalidated_records` 只统计实际退出 active 的记录；
- `pinned_skipped` 统计本应被 cooling/archive/invalidate/merge 处理但因 Pinned 跳过的记录；
- `derived_repairs` 统计本次通过 `index_jobs` 修复的 text/vector 派生更新；
- provider 未配置时，report 必须区分“规则维护已执行”和“自动结构化未执行”。

## 派生层维护流程

### 当前遗留 restore 增量流程

当前 `restore(scope)`：

```text
load outstanding index_jobs
  -> pending jobs 优先，若全 failed 才处理 failed
  -> load SQLite document per job
  -> build TextUpdate/VectorUpdate
  -> apply_updates
  -> clear jobs
  -> record index ready
```

失败：

- fail index jobs；
- attempts + 1；
- last_error 写入；
- index_state 标记 failed。

### 当前遗留 restore_full 流程

当前 `restore_full(scope)`：

```text
if text:
  load active episodes/entities/facts as search docs
  Tantivy full refresh
  clear all text jobs
  record text ready

if vector:
  load active episodes/entities/facts with vector_json
  HNSW full refresh
  clear all vector jobs
  record vector ready

refresh L3 cache
```

最终标准：

- 删除公开 `restore` 命令；
- 不新增 `rebuild` 命令；
- 派生层维护统一归入 `dream`；
- `dream` 可以在日常维护中 best-effort 处理常规派生修复，消费现有 `index_jobs` 对 text/vector 派生层执行增量 apply；
- `dream` 可以按 SQLite 真相源刷新 text/vector 派生层；
- `index_jobs` 只保留为可观测的修复账本和异常诊断材料，不包装成后台队列、worker 或 daemon。

`dream` 常规修复边界：

```text
load pending/failed index_jobs
  -> group by index_name
  -> read each job target from SQLite truth source
  -> active truth record becomes upsert update
  -> archived/invalidated/missing record becomes delete update
  -> apply text/vector updates
  -> clear successful jobs
  -> keep failed jobs with attempts/last_error
  -> never create memory records
  -> never call provider for pure index repair
```

说明：上面的 pure index repair 只指根据已有 SQLite material 修复 text/vector 派生层。`dream` 在同一次显式维护命令中仍可以执行 extraction 和 embedding，把结构化记录和 `vector_json` 写回 SQLite；随后再把这些本地材料同步到派生层。

派生层刷新边界：

```text
load active episodes/entities/facts for text
  -> refresh Tantivy from SQLite truth
  -> load active episodes/entities/facts where vector_json is not null
  -> refresh HNSW/vector manifest from SQLite truth
  -> record index_state ready
  -> clear related index_jobs
```

pure index repair 明确不做：

- 不读取 unstructured episode 去 extraction；
- pure index repair 不补缺失 `vector_json`；
- 不根据 hit count 晋升；
- 不冷却 L3；
- 不处理冲突；
- 不更新 Working Set；
- 不改 Pinned；
- 不把 failed provider runtime 清成 ok。

## State 判断流程

当前输入：

- engine state；
- provider runtime summary；
- provider readiness summary。

当前 engine state 包含：

- episode/entity/fact/edge count；
- unstructured L1/L2；
- structured total；
- anchored records；
- layer summary；
- l3 cached；
- text index status；
- vector index status。

最终 state 计算：

```text
read SQLite counts/layers/lifecycle
  -> read Working Set/Pinned counts
  -> read text/vector index_state + jobs
  -> read provider readiness
  -> read provider runtime
  -> compute capability statuses
  -> compute internal diagnostics when needed
  -> emit status, message, next
```

能力状态展示规则：

- 能力状态可以多值同时展示，不强行压成单枚举；
- `not_ready` 优先级最高，只要 extraction provider 未配置、仍是 placeholder，或配置读取失败，就必须出现；
- runtime `degraded` 不等于未配置，但必须降低对应能力的健康状态，并在 detail 中显示最近错误；
- `configured` 只表示配置看起来完整，不等于远程 provider 已验证可用；
- `structure_ready` 要求 extraction readiness 至少为 `configured`，且没有配置读取错误；
- `semantic_ready` 要求本地已有可用结构化数据，并且 vector index 的 `index_state.status = ready` 且 `doc_count > 0`；ready 但空的 vector index 只能说明派生层一致，不能宣称本地语义召回可用。

动作提示规则：

- 普通输出只有 `status`、`message`、`next`；
- `status` 只能是 `ready`、`needs_setup`、`needs_dream`；
- `next` 只能是 `none`、`configure provider`、`memo dream`；
- extraction 未配置或 placeholder：`status=needs_setup`，`next=configure provider`，message=`需要先配置 provider`；
- 有未整理内容、缺少本地语义材料、派生层未同步或派生层不可信：`status=needs_dream`，`next=memo dream`，message=`有新内容需要整理`；
- provider degraded：不自动设置 dream 动作，但在 provider detail 中显示最近错误；
- Pinned/Working Set 只展示状态，不制造维护任务。

输出约束：

- text 输出优先给用户可执行动作，不要求用户理解 `index_jobs` 或 `index_state`；
- json 输出可以保留结构化 `diagnostics`，但普通输出不展示；
- `diagnostics.internal_reasons` 可以包含内部原因，例如 `needs_structure`、`needs_vectors`、`sync_needed`、`full_refresh_needed`；
- 内部诊断如果需要暴露表名，应放到 `diagnostics` 下，不能混入主状态行；
- 无论内部有多少状态组合，`next` 同一时间只给一个主动作。

## Eval、Bench 与质量模型

### Eval dataset

当前 `EvalDataset`：

- `name`；
- `memories`；
- `cases`。

`EvalMemory`：

- id；
- content；
- entities；
- facts；
- session_id；
- recorded_at；
- confidence。

`EvalCase`：

- id；
- aspect；
- query；
- expected_memory_ids；
- forbidden_memory_ids；
- limit；
- deep；
- should_abstain；
- dream_before_recall。

还支持 normalized public JSONL：

- `type=memory`；
- `type=query`。

### Eval 流程

当前 `run_recall_eval`：

```text
load eval memories via engine.remember
  -> refresh text derived layer
  -> for each case:
       optionally dream
       optionally refresh text derived layer
       run recall
       collect traces
  -> summarize metrics
```

注意：

- 当前 eval 初始只刷新 text 派生层；
- vector 质量不一定在默认 eval 中完整覆盖；
- dream_before_recall 可触发 dream。

### Eval 指标

当前 report 指标：

- recall_at_1；
- recall_at_5；
- source_recall_at_1；
- source_recall_at_5；
- MRR；
- source MRR；
- expected_hit_rate；
- clean_hit_rate；
- successful_case_rate；
- precision_at_1；
- precision_at_5；
- clean_precision_at_5；
- forbidden_rate；
- noise_hit_rate；
- mean_source_diversity；
- mean_duplicate_rate；
- abstention_correctness；
- forbidden_correctness；
- timing；
- kind_counts；
- failure_mode_counts；
- aspect reports；
- case traces。

失败模式：

- non_abstained；
- missed_expected；
- forbidden_hit；
- missing_expectation。

### Bench

当前 bench：

- `recall_latency.rs`；
- `recall_quality.rs`；
- `benches/support/latency.rs`；
- `benches/support/quality.rs`。

synthetic datasets：

- `basic.json`；
- `smoke.json`；
- `quality.json`；
- `temporal.json`；
- `stress.json`；
- `adversarial.json`；
- public eval README。

最终标准：

- recall 排序变化必须跑质量评估或补行为测试；
- 默认 recall provider_calls=0 必须有 mock provider 调用计数测试；
- L2 > 弱相关 L3 必须有回归测试；
- Working Set 加权必须有“不压过明确匹配”的测试；
- Pinned dream 保护必须有测试；
- dream 派生层维护不应破坏 layer/status/Working Set/Pinned，必须有测试。

## Extraction adapter 设计

当前 root adapter：

- prompt 构建在 `src/providers/adapters/extraction/prompt.rs`；
- normalize 在 `normalize.rs`；
- adapter 在 `adapter.rs`；
- extraction cleanup 参数来自 config；
- engine 只接收 normalized `ExtractionResult`。

最终标准：

- provider prompt 和 cleanup 属于 root/provider adapter；
- engine 不持有 prompt 模板；
- extraction result 必须被 normalize 后再进入 engine；
- provider 原始响应不应污染 SQLite schema。

## CLI 输出设计

当前输出：

- 支持 text/json；
- recall 输出结果、score、reason；
- dream 输出 report；
- 当前不再公开 restore 输出；
- state 输出 engine/provider 状态。

最终标准：

- 默认 recall 输出能力位；
- state 普通输出只显示 `status`、`message`、`next`；JSON 可附带 `diagnostics`；
- dream 输出 provider 缺失/失败和整理结果区别；
- 不再输出或主推恢复类命令；
- 错误信息必须区分 SQLite 写失败、provider 未配置、provider degraded、派生层需要 dream 维护。

## 分发、模板与外部集成

当前项目不只有 engine 和 CLI 代码，还包含安装、模板、全局 home 行为和外部工具集成材料。这些内容也属于最终设计边界。

### Workspace 元数据

当前根目录包含：

- `Cargo.toml`：workspace 和根 CLI crate 配置；
- `Cargo.lock`：可重复构建锁定；
- `LICENSE`：授权文件；
- `AGENTS.md`：仓库内协作规范，不属于产品运行时。

最终标准：

- workspace 结构变化必须同步本文的模块边界；
- 新增 crate 或 bin 不能绕开 `memo` / `memo-engine` / `lmkit` 的职责边界；
- 发布或安装说明不得领先实际 artifact。

### 配置模板

当前模板：

- `templates/config.toml`；
- `templates/providers.toml`。

当前语义：

- `config.toml` 默认写 provider ref，例如 `openai.embed`、`openai.extract`；
- `providers.toml` 默认包含 OpenAI、Ollama，以及注释中的 Aliyun、Google、Zhipu 示例；
- OpenAI 默认 api key 是 placeholder；
- Ollama 允许空 api key；
- `awaken` 只写缺失模板，不覆盖用户配置。

最终标准：

- 模板是引导材料，不等于 readiness；
- placeholder key 等同未配置；
- 新增 provider 示例必须同步 readiness 检测和配置 parser 测试；
- 模板注释必须与 provider 调用边界一致，不能暗示默认 recall 会现场联网。

### 安装脚本

当前安装脚本：

- `scripts/install.sh`；
- `scripts/install.ps1`。

最终标准：

- 安装脚本只负责安装 CLI artifact 和必要文件；
- 不初始化用户记忆库，初始化由 `memo awaken` 显式完成；
- 安装路径、shell profile 修改和 PATH 提示必须与 README 保持一致；
- 安装脚本不得创建后台服务、计划任务、daemon 或 worker。

### 全局 home 行为测试

当前测试：

- `tests/global_home_cli.rs`。

最终标准：

- 该测试是 CLI 默认 home/config/data 语义的验收入口；
- 修改默认路径、`MEMO_DATA_DIR`、`awaken` 行为或模板写入时必须同步该测试；
- 文档中的路径示例必须与该测试和 `src/cli/paths.rs` 一致。

### Cursor hook

当前外部集成：

- `hooks/Cursor/hooks.json`。

最终标准：

- hook 只能作为外部编辑器集成材料；
- hook 不得引入后台记忆整理语义；
- hook 调用 CLI 时必须遵守公开命令契约，尤其默认 recall 不调用 provider。

## 文档体系

当前文档：

- `README.md`；
- `docs/COMMANDS.md`；
- `docs/zh-CN/README.md`；
- `docs/zh-CN/COMMANDS.md`；
- `docs/architecture/memory-engine-architecture.md`；
- `docs/architecture/command-philosophy.md`；
- `docs/architecture/final-standard-design.md`；
- `docs/superpowers/specs/*`；
- `docs/superpowers/plans/*`；
- `evals/README.md`；
- `evals/public/README.md`；
- `skills/memo-brain/*`；
- `crates/lmkit/docs/*`。

当前分发和集成材料：

- `templates/config.toml`；
- `templates/providers.toml`；
- `scripts/install.sh`；
- `scripts/install.ps1`；
- `tests/global_home_cli.rs`；
- `hooks/Cursor/hooks.json`；
- `Cargo.toml`；
- `Cargo.lock`；
- `LICENSE`。

最终职责：

- 本文档：项目总设计基线；
- `memory-engine-architecture.md`：实现架构和流程图，应按本文收敛；
- `command-philosophy.md`：公开命令命名哲学，应删除恢复类命令心智；
- `docs/COMMANDS.md` / `docs/zh-CN/COMMANDS.md`：当前已实现命令手册，不写未实现能力；
- README：用户入口，只写当前可用能力和明确目标差异；
- skills：外部 agent 使用手册，必须避免旧 `search` / `embed` / 恢复类命令口径漂移；
- lmkit docs：保持独立库文档，不混入 memo-brain 记忆模型。

## 当前已知实现差距

需要按最终标准收敛的差距：

- 当前 `dream` 的 embedding 职责已进入维护路径，但 provider call 统计输出还不完整；
- 当前 recall 输出已稳定展示 `provider_calls=0`、`total_candidates` 和 candidate `capabilities` 诊断语义，后续文档需保持同一契约；
- 当前 L0/session cache 仍出现在 recall reason；
- 当前 `anchored_at` 仍保留为兼容字段，对外命名还未完全统一成 Pinned；
- 当前 Pinned 没有完整 recall 加权语义；
- README、COMMANDS、中文文档、skill、安装脚本说明和模板注释尚未同步本文的默认路径、Working Set、Pinned 和 provider 边界；
- 其他文档和分发材料仍有旧恢复命令、L0/session、provider 可选增强等口径漂移风险。

## 最终验收标准

命令边界：

- `awaken` 不调用 provider；
- `remember` 不调用任何 provider；
- 默认 `recall` 不调用任何 provider；
- `dream` 可以调用 extraction 和 embedding provider；
- 不存在公开 `restore` 或 `rebuild` 命令。

数据语义：

- SQLite 是唯一真相源；
- L1/L2/L3 是成熟度轴；
- Working Set 是持久化活跃度轴；
- Pinned 是保护轴；
- Archived/Invalidated 是生命周期状态；
- text/vector/cache/index state 都不是记忆层。

召回质量：

- active L1/L2/L3 都可 recall；
- L2 强匹配优先于弱相关 L3；
- L1 作为证据兜底；
- L3 只提供稳定性和热度加权；
- Working Set 不压过明确匹配；
- Pinned 不强推不匹配内容；
- vector 只使用已有本地向量；
- 默认输出 `provider_calls=0`。

维护语义：

- `dream` 做日常语义整理、向量补齐和派生层维护；
- `index_jobs` 是修复账本，不是后台队列。
- 不新增公开增量 repair 命令或恢复命令；公开维护心智只保留 `dream`。

状态语义：

- 必要 provider 未配置或未就绪时，`state` 主状态为 `needs_setup`，provider capability diagnostics 显示 `not_ready`；
- extraction 静态配置满足 `dream` 前置条件时显示 `structure_ready`，但 runtime 是否健康必须单独展示；
- 结构化数据和本地 vector 派生层可用时显示 `semantic_ready`；
- 常规整理、向量补齐、派生层同步或刷新需要 dream 时，普通输出统一为 `status=needs_dream`、`next=memo dream`；
- 内部 `diagnostics.internal_reasons` 可以覆盖 `needs_structure`、`needs_vectors`、`sync_needed`、`full_refresh_needed`；
- 能力状态可以多值同时展示，`not_ready` 不能被其他 ready 状态掩盖。

路径与分发：

- 默认配置目录是 `~/.memo`；
- 默认数据目录是 `~/.memo/data`；
- `MEMO_DATA_DIR` 优先于 `[storage].data_dir`；
- 安装脚本不得创建后台服务或隐式初始化记忆库；
- 模板 placeholder 不能被文档描述为已配置 provider。

## 推荐实施顺序

后续代码改造建议按以下顺序推进：

- 继续补 provider call 统计输出；
- 继续收敛公开文档、README、skill 和模板口径；
- 将 anchored CLI/output/state 语义收敛为 Pinned；
- 修改 recall 排序，保证 L2 强匹配优先于弱相关 L3；
- 更新 README、COMMANDS、中文文档、架构文档、skill、安装脚本说明和模板注释；
- 补全或更新 `tests/global_home_cli.rs` 覆盖默认路径和 `MEMO_DATA_DIR`；
- 跑 `cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo build --all-features`、`cargo test --all-features`。

## 文档维护规则

后续更新本文时必须遵守：

- 不用未实现目标冒充当前能力；
- 数据表字段变化必须同步本设计；
- recall/dream 算法变化必须同步本设计；
- provider 调用边界变化必须同步测试和本文；
- CLI 命令增删改必须同步 README、COMMANDS、中英文文档和 skill；
- Markdown 标题不使用序号。
