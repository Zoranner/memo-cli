# 命令参考

当前本地单进程记忆引擎的命令参考。

中文 | [English](../COMMANDS.md)

## 公开命令

[命令设计哲学](../architecture/command-philosophy.md) 定义了公开命令语言标准。本文档直接遵循这套公开命令面。

- `memo awaken`
- `memo remember`
- `memo recall`
- `memo reflect`
- `memo dream`
- `memo state`

---

## `memo awaken`

初始化数据目录，并写入配置模板。

### 语法

```bash
memo awaken
```

### 输出

输出一段人类可读摘要，包含：

- 唤醒的目录
- 固定配置目录
- `config.toml` 是新建还是保留
- `providers.toml` 是新建还是保留

### 说明

- `memo awaken` 会始终把 `config.toml` 与 `providers.toml` 固定保存在 `~/.memo`
- 默认数据目录是 `~/.memo/data`
- 如需覆盖数据目录，可设置环境变量 `MEMO_DATA_DIR`，或在 `~/.memo/config.toml` 中设置 `storage.data_dir`

---

## `memo remember`

把一条 episode 写入 SQLite；如派生索引需要维护，由 `memo state` 提示后续运行 `memo dream`。

### 语法

```bash
memo remember <content> [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `--time <rfc3339>` | 观测时间 |
| `--entity <type:name[:alias1|alias2]>` | 手动补充实体 |
| `--fact <subject:predicate:object>` | 手动补充事实 |
| `--json` | 输出机器可读结果 |

### 说明

- 默认 `memo remember` 只会立即写入手工 entities 和 facts
- `--entity` 和 `--fact` 是高级结构化入口；普通用户可以只写自然语言 episode，等配置 extraction 后由 `memo dream` 慢路径补结构化
- 默认情况下，其它命令使用 `~/.memo/data`；`MEMO_DATA_DIR` 优先于 `storage.data_dir`，而 `storage.data_dir` 优先于默认值

---

## `memo recall`

查询引擎。默认先走快路径；如果结果看起来不确定，系统可能自动升级成 deep search。

### 语法

```bash
memo recall <query> [OPTIONS]
```

### 选项

| 选项 | 说明 |
| --- | --- |
| `-n, --limit <n>` | 结果上限，默认 `10` |
| `--deep` | 直接强制启用深搜 |
| `--json` | 输出机器可读结果 |

### 说明

- 默认 recall 读取本地记忆状态，不应要求 provider 调用
- 输出里包含 `deep_search_used` 和每条结果的 `reasons`

---

## `memo reflect`

按 id 查看单条记忆记录。

### 语法

```bash
memo reflect <id> [--json]
```

---

## `memo dream`

dream 入口。

### 语法

```bash
memo dream [--full] [--json]
```

### 行为

- 默认执行一次手动 dream
- `--full` 会执行更完整的一次 dream；当第一次整理改变了记忆状态时，会追加一次稳定化 pass
- 配置了 extraction provider 时，dream 可以在慢路径补齐仍未结构化的 episode，而不会改变 `remember` 的默认延迟边界
- 如果 extraction 未配置、不可用，或仍是模板占位 key，dream 会明确报告仍有 episode 只能作为文本记忆保留，而不是假装已经语义整理
- dream 也是公开的 text/vector 派生层维护入口；内部修复细节只作为诊断信息，不单独暴露成用户流程
- 文本输出包含 `provider_extraction_calls`、`provider_embedding_calls` 和 `pinned_skipped`
- `--json` 输出机器可读结果，包含 `dream.provider_calls.extraction_calls`、`dream.provider_calls.embedding_calls` 和 `dream.pinned_skipped`

---

## `memo state`

### 语法

```bash
memo state [--json]
```

### 文本输出

文本输出只暴露面向用户的动作契约：

- `status`：只能是 `ready`、`needs_setup`、`needs_dream`
- `message`：简短原因
- `next`：只能是 `none`、`configure provider`、`memo dream`

provider 未配置、未就绪或仍使用模板占位 key 时，输出 `status: needs_setup` 和 `next: configure provider`。

存在未整理内容、缺少本地语义材料，或派生层未同步/不可信时，输出 `status: needs_dream` 和 `next: memo dream`。

### JSON 输出

`--json` 保留同样的顶层 `status`、`message`、`next`，并增加 `diagnostics`：

- `diagnostics.internal_reasons`，例如 `provider_not_ready`、`needs_structure`、`needs_vectors`、`sync_needed`、`full_refresh_needed`
- engine state 计数与索引状态
- provider readiness 和 runtime health

`index_jobs` / `index_state` 等内部账本只属于 diagnostics，不进入文本主状态行。

