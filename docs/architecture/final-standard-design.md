# memo-brain 最终标准设计

**日期：** 2026-05-07  
**范围：** 命令职责、provider 边界、记忆层级、Working Set、派生层维护、数据库读写

## 文档目的

本文档定义 memo-brain 后续实现应对齐的目标契约。

它回答三类问题：

- 公开命令分别负责什么；
- provider、SQLite 真相源和派生索引之间如何分工；
- 记忆在数据库中如何写入、整理、查询和重建。

本文档是新的标准设计。若旧架构文档、命令文档或当前代码与本文冲突，应以本文作为后续改造依据。

## 核心结论

memo-brain 是显式 CLI 记忆引擎，不引入后台、worker、daemon 或自动调度。

provider 是正常智能使用的前置条件，但 provider 调用不能进入默认 `recall`。智能能力必须先通过显式维护命令沉淀到 SQLite 和本地派生层，查询时只读本地数据，保证稳定快。

核心边界如下：

- `remember` 只负责快速写真相源；
- `dream` 负责日常语义整理与收敛；
- `rebuild` 负责从 SQLite 全量重建派生层；
- `recall` 默认只读本地数据，不调用 provider；
- `state` 必须诚实展示系统是否已经具备智能可用性。

## 记忆模型

### 成熟度层级

成熟度层级只保留三层：

- `L1 Evidence`：原始证据层，保存用户写入的 episode 和低整理度内容，是文本证据和兜底来源；
- `L2 Structured`：结构化主工作层，保存 entity、fact、edge，是默认 recall 的主力来源；
- `L3 Stable`：长期稳定层，表示经过支持度、命中频率或时间跨度验证的稳定记忆。

`L3` 只提供稳定性和热度加权，不能让弱相关内容压过更匹配的 `L2`。

### Working Set

`Working Set` 是短期活跃横切视图，不是 `L0`，也不是进程内 session cache。

它表示最近一段时间内活跃的 active records，包括：

- 最近写入的记忆；
- 最近 `recall` 命中的记忆；
- 最近 `reflect` 查看过的记忆；
- `dream` 新生成或更新的结构化记忆。

Working Set 必须持久化在 SQLite 中，跨 CLI 命令有效。默认窗口建议为 7 天。

Working Set 横切 `L1 / L2 / L3`。一条记忆可以同时是 `L2` 和 Working Set，也可以同时是 `L3` 和 Working Set。

### 生命周期状态

生命周期状态不是 L 维度。

- `active`：参与默认 recall；
- `archived`：归档，不参与默认 recall，但保留在 SQLite 真相源中；
- `invalidated`：失效，不参与默认 recall，但保留用于解释冲突来源。

`archived` 和 `invalidated` 不是 `L4`。它们横切 `L1 / L2 / L3`。

### 派生服务层

以下内容不是记忆层：

- Tantivy text index；
- local vector index；
- L3 preload cache；
- index state；
- index recovery bookkeeping。

这些都是 SQLite 真相源的派生层，可从真相源重建。

## 数据库模型

### 业务真相表

继续保留四类业务真相表：

- `episodes`
- `entities`
- `facts`
- `edges`

它们保存具体内容、时间、置信度、source linkage、hit count、archived/invalidated 时间等业务事实。

### 层级元数据表

`memory_layers` 是跨 record kind 的层级与状态元数据表，应成为成熟度、生命周期和 Working Set 的统一元数据入口。

建议目标结构：

```sql
memory_layers(
  memory_id TEXT NOT NULL,
  memory_kind TEXT NOT NULL,
  layer TEXT NOT NULL,
  status TEXT NOT NULL,
  last_promoted_at INTEGER NULL,
  last_l3_promoted_at INTEGER NULL,
  working_set_at INTEGER NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(memory_id, memory_kind)
)
```

字段语义：

- `layer`：`L1`、`L2`、`L3`；
- `status`：`active`、`archived`、`invalidated`；
- `working_set_at`：最近进入 Working Set 的时间；
- `last_promoted_at`：最近一次层级晋升时间；
- `last_l3_promoted_at`：最近一次进入 L3 的时间，用于 L3 冷却和重复提升控制；
- `updated_at`：该元数据行最近变化时间。

如果当前实现已有 `anchored_at`，本轮标准设计不继续扩展它。Pinned/Core 保护语义应作为后续独立设计处理，避免与本轮 Working Set 和成熟度改造混杂。

### 派生层状态表

`index_state` 记录派生层整体状态，例如：

- `text` index 是否 ready；
- `vector` index 是否 ready；
- doc count；
- last rebuilt time；
- detail。

`index_jobs` 可以保留，但语义必须收敛为“派生状态未同步或修复账本”，不是后台队列，也不是常规 `rebuild` 输入。

正常情况下 `index_jobs` 应尽量为空。只有派生层同步失败、索引状态不可信或需要后续修复时才写入。

## 命令职责

### awaken

职责：初始化配置模板和本地空间。

行为：

- 创建或确认配置目录；
- 创建或确认数据目录；
- 写入缺失的配置模板；
- 不调用 provider；
- 不验证远程服务。

输出应能提示用户 provider 未配置时系统仍处于 `not_ready`。

### remember

职责：快速写入 SQLite 真相源。

读：

- 读取本地配置以定位数据目录；
- 不读取 provider 以执行模型调用。

写：

1. 插入 `episodes`；
2. 插入对应 `memory_layers`：
   - `layer = L1`
   - `status = active`
   - `working_set_at = now`
3. 如果用户显式传入 `--entity` 或 `--fact`：
   - 写入或更新 entity、fact、edge；
   - 对新增或更新记录写入或更新 `memory_layers.working_set_at = now`；
4. 尽量同步本地 text index；
5. 如果本地派生层更新失败，记录到 `index_jobs`，但不回滚 SQLite 真相源。

禁止：

- 不调用 extraction provider；
- 不调用 embedding provider；
- 不调用 rerank provider；
- 不执行 dream 整理；
- 不做自动语义补全。

失败边界：

- SQLite 写入失败：命令失败；
- 派生索引更新失败：命令成功，但 `state` 显示 `maintenance_needed`；
- provider 不可用：不影响 `remember`，因为它不调用 provider。

### dream

职责：日常语义整理与收敛。

读：

- active 的未结构化 `L1 / L2` episodes；
- 支持度、重复、冲突相关记录；
- `L2 / L3` 中需要复核的 active records；
- 常规派生状态未同步记录。

写：

1. 有 extraction provider 时，对未结构化 episode 做抽取；
2. 写入 entity、fact、edge；
3. 新生成或更新的结构化记录写入：
   - `layer` 按整理规则确定；
   - `status = active`；
   - `working_set_at = now`；
4. 标记 episode `structured_at`；
5. 推进 `L1 -> L2`、`L2 -> L3`；
6. 对冲突事实执行 invalidation；
7. 对重复或被合并内容执行 archive；
8. 对 stale L3 执行冷却，只允许 `L3 -> L2`，不删除；
9. 修正常规派生状态；
10. 刷新必要的本地 cache。

provider 边界：

- extraction provider 只允许在 `dream` 中调用；
- embedding provider 不属于 `dream` 的默认职责；
- rerank provider 不属于 `dream` 的默认职责。

无 extraction provider 时：

- 不应假装语义整理完成；
- 报告系统处于 `not_ready` 或 `structure_not_ready`；
- 可以执行不依赖 provider 的规则维护，但输出必须区分“规则维护完成”和“自动结构化未执行”。

### rebuild

职责：从 SQLite 真相源全量重建派生层。

`rebuild` 替代 `restore` 成为主命令名。

读：

- 所有 `status = active` 的 episode、entity、fact；
- text document 所需字段；
- 已存在的 `vector_json`。

写：

1. 清空并重建 Tantivy text index；
2. 清空并重建 local vector index；
3. 更新 `index_state`：
   - `status = ready`
   - `doc_count = rebuilt count`
   - `last_rebuilt_at = now`
4. 清理对应 `index_jobs`。

禁止：

- 不调用 extraction；
- 不调用 embedding 补缺失向量；
- 不调用 rerank；
- 不改变 memory layer；
- 不改变 memory status；
- 不做语义整理；
- 不做事实合并或冲突判断。

旧 `restore`：

- 保留为兼容 alias；
- 行为等价于 `rebuild`；
- 文档主推 `rebuild`。

### recall

职责：默认纯本地快查询。

默认 `recall` 必须满足：

- `provider_calls = 0`；
- 不调用 extraction；
- 不调用 embedding；
- 不调用 rerank；
- 不依赖进程内 session 作为 CLI 产品语义。

候选来源：

1. SQLite exact / alias；
2. Tantivy text index；
3. local vector index 中已有向量材料；
4. graph relations；
5. L3 preload cache；
6. Working Set。

如果 query embedding 需要远程 embedding provider，默认 recall 不走 vector query。向量召回只能使用已经本地化、不会触发 provider 的材料。

打分原则：

- `L2` 是主力结构化召回来源；
- `L1` 是原始证据兜底；
- `L3` 是稳定性和热度加权，不能压过更匹配的 `L2`；
- Working Set 只做小幅最近活跃加权；
- subject mismatch 只能作为解释或小幅降权，不能成为脆弱的硬过滤；
- graph expansion 默认受显式语义开关控制，不能靠 `deep` 或 `limit` 隐式扩大输出。

写：

1. 对最终返回结果更新 hit count；
2. 更新 `memory_layers.working_set_at = now`；
3. 不写 provider runtime；
4. 不触发模型调用。

输出建议包含本次能力摘要：

```text
provider_calls=0 text=true vector=true l1=true l2=true l3=true working_set=true
```

### reflect

职责：查看单条 SQLite 真相源记录。

读：

- 按 id 查找 episode、entity、fact、edge；
- 可返回 archived 或 invalidated 记录，帮助用户理解历史状态。

写：

- 若记录存在，更新 `memory_layers.working_set_at = now`；
- 不改 layer；
- 不改 status；
- 不调用 provider。

### state

职责：诚实展示系统是否可用、哪里未就绪、下一步应执行什么命令。

输出应包含：

- `capability_mode`；
- provider readiness；
- L1/L2/L3 active counts；
- archived / invalidated counts；
- structured / unstructured episode counts；
- Working Set 窗口和 active count；
- text/vector index 状态；
- index repair bookkeeping 数量；
- 是否需要 `dream`；
- 是否需要 `rebuild`。

## Provider 契约

provider 是正常智能使用前置条件，不是普通增强项。

### extraction provider

必须用于自动结构化。

- 只允许 `dream` 调用；
- 无 extraction provider 时，系统不能宣称智能记忆可用；
- placeholder key 等同未配置。

### embedding provider

必须用于形成语义向量材料，但默认 recall 不现场调用。

- 默认 `remember` 不调用；
- 默认 `recall` 不调用；
- `rebuild` 不调用它补缺失向量；
- 向量必须提前沉淀为本地材料后才能参与默认 recall。

缺失向量补齐应作为后续独立显式能力设计，不能塞进 `rebuild` 或默认 `recall`。

### rerank provider

不进入默认 recall。

如果保留，只能作为显式慢选项或离线评估能力，并且必须在输出中明确标记 provider 调用。

## 能力模式

### not_ready

必要 provider 未配置或仍是 placeholder。

系统只能记录原文和查看状态，不承诺智能记忆可用。

### structure_ready

extraction provider 可用。

`dream` 可以把自然语言 episode 转成结构化 entity/fact/edge。

### semantic_ready

结构化数据和本地向量派生层可用。

默认 `recall` 可以走本地语义召回，但仍不现场调用 provider。

### maintenance_needed

常规派生状态或结构化状态需要 `dream` 收敛。

典型原因：

- 有未结构化 episode；
- 常规派生状态未同步；
- 有可整理的重复、冲突或 stale L3。

### rebuild_needed

派生索引丢失、损坏或不可信，需要 `rebuild` 全量重建。

## 数据流

### 写入到查询

```text
remember
  -> SQLite: episode / manual entity / manual fact
  -> memory_layers: active L1 + working_set_at
  -> local text index best-effort sync
  -> recall can read SQLite truth immediately
```

### 整理到查询

```text
dream
  -> extraction provider
  -> SQLite: entity / fact / edge
  -> memory_layers: L1/L2/L3 + working_set_at
  -> archive / invalidate / cool
  -> local derived state repair
  -> recall reads structured local truth
```

### 重建派生层

```text
rebuild
  -> read active SQLite truth
  -> rebuild text index
  -> rebuild vector index from existing vector_json
  -> update index_state
  -> clear index repair bookkeeping
```

### 查询

```text
recall
  -> SQLite exact / alias
  -> text index
  -> local vector index if available without provider call
  -> graph relations
  -> L3 cache
  -> Working Set boost
  -> update hit_count + working_set_at
```

## 测试要求

必须覆盖以下行为：

- `remember` 不调用 extraction、embedding、rerank；
- 默认 `recall` 不调用 extraction、embedding、rerank；
- `dream` 只调用 extraction provider；
- `rebuild` 不调用任何 provider；
- `rebuild` 不改变 layer 和 status；
- `restore` alias 与 `rebuild` 等价；
- `L2` 结构化结果能被 recall 查到，并优先于弱相关 `L3`；
- Working Set 跨 CLI 进程有效；
- `reflect` 会让记录进入 Working Set；
- 无 provider 时 `state` 显示 `not_ready`；
- 无 extraction provider 时 `dream` 明确报告无法自动结构化；
- `rebuild` 成功后清理对应 index repair bookkeeping。

## 实施顺序

推荐按以下顺序实施：

1. 补 provider 调用边界测试；
2. 将 `restore` 主语义替换为 `rebuild`，保留 alias；
3. 增加 `working_set_at` schema、迁移和 DB API；
4. 修改 `remember`，移除默认 embedding 调用；
5. 修改 `recall`，移除默认 query embedding 和 rerank；
6. 修改 `reflect` / `recall` / `dream` 的 Working Set 更新；
7. 修改 `state` 能力模式和维护提示；
8. 更新 README、命令文档和旧架构文档中的冲突口径。
