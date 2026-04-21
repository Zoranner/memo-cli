# 命令参考

当前本地单进程记忆引擎的命令参考。

中文 | [English](../COMMANDS.md)

## 可用命令

- `memo init` — 初始化本地配置模板
- `memo extract` — 直接调用抽取 provider
- `memo ingest` — 把一条 episode 写入 SQLite 真相源
- `memo query` — 通过快路径或深搜查询引擎
- `memo inspect` — 按 id 查看单条记忆记录
- `memo dream` — 立即执行或排队 consolidation 任务
- `memo run-dream-jobs` — 消费已排队的 consolidation 任务
- `memo refresh-index` — 只刷新处于 pending 的派生索引
- `memo rebuild-index` — 强制全量重建派生索引
- `memo stats` — 查看引擎与任务队列状态
- `memo benchmark` — 重复执行查询基准

---

## `memo init`

初始化数据目录，并写入配置模板。

### 语法

```bash
memo init [--data-dir <path>]
```

### 输出

返回 JSON，包含：

- `data_dir`
- `config_created`
- `providers_created`

---

## `memo extract`

直接调用配置好的抽取 provider，不写入记忆。

### 语法

```bash
memo extract <content> [--data-dir <path>]
```

### 说明

- 需要在 `config.toml` 中配置 `[extract] extraction_provider`
- 返回抽取出的 entities 和 facts 的格式化 JSON

---

## `memo ingest`

把一条 episode 写入 SQLite，同时更新 L0/session 状态，并把派生索引标记为 `pending`。

### 语法

```bash
memo ingest <content> [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `--layer <L1|L2|L3>` | 目标记忆层级，默认 `L1` |
| `--session <id>` | 可选 session id |
| `--at <rfc3339>` | 可选观测时间 |
| `--entity <type:name[:alias1|alias2]>` | 手动补充实体 |
| `--fact <subject:predicate:object>` | 手动补充事实 |
| `--dry-run` | 只预览最终 ingest payload，不落库 |

### 说明

- `--dry-run` 会输出手工输入和 provider 抽取合并后的最终内容
- 实际索引刷新现在是显式步骤；`ingest` 只会把 `text` / `vector` 索引标成 `pending`

---

## `memo query`

查询引擎。默认先走快路径；如果结果看起来不确定，系统可能自动升级成 deep search。

### 语法

```bash
memo query <query> [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `-n, --limit <n>` | 结果上限，默认 `10` |
| `--deep` | 直接强制启用深搜 |

### 说明

- 配置了 rerank 时，deep search 可以触发精排
- 输出里包含 `deep_search_used` 和每条结果的 `reasons`

---

## `memo inspect`

按 id 查看单条记忆记录。

### 语法

```bash
memo inspect <id> [--data-dir <path>]
```

---

## `memo dream`

consolidation 入口。

### 语法

```bash
memo dream [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `--trigger <manual|idle|session_end|before_compaction>` | consolidation 触发器，默认 `manual` |
| `--enqueue` | 只排队，不立即执行 |

### 行为

- 默认直接同步执行 consolidation
- 加上 `--enqueue` 后，只返回一个 pending 的 `job_id`

---

## `memo run-dream-jobs`

消费已排队的 consolidation 任务。

### 语法

```bash
memo run-dream-jobs [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `--limit <n>` | 最多执行多少个排队任务，默认 `1` |

---

## `memo refresh-index`

只刷新当前被标记为 pending 的索引。

### 语法

```bash
memo refresh-index [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `--scope <all|text|vector>` | 刷新范围，默认 `all` |

### 说明

- 这是 `ingest` 之后的常规维护入口
- 如果目标索引当前不是 `pending`，会返回空的 rebuild report

---

## `memo rebuild-index`

从 SQLite 真相源强制全量重建派生索引。

### 语法

```bash
memo rebuild-index [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `--scope <all|text|vector>` | 重建范围，默认 `all` |

### 说明

- 适用于损坏恢复或显式全量重建
- 与 `refresh-index` 不同，它不要求索引先处于 `pending`

---

## `memo stats`

查看引擎状态。

### 语法

```bash
memo stats [--data-dir <path>]
```

### 包含内容

- episode / entity / fact / edge 数量
- L3 cache 大小
- text / vector 索引状态
- consolidation 任务计数（`pending`、`running`、`completed`、`failed`）

---

## `memo benchmark`

重复执行查询基准。

### 语法

```bash
memo benchmark <query> [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--data-dir <path>` | 指定数据目录 |
| `--iterations <n>` | 执行次数，默认 `20` |
| `-n, --limit <n>` | 查询结果上限，默认 `10` |

### 说明

- 当前 benchmark 默认跑非强制 deep 的查询
- 输出包含平均耗时和总耗时（毫秒）
