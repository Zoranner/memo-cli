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
- `memo restore`

---

## `memo awaken`

初始化数据目录，并写入配置模板。

### 语法

```bash
memo awaken [path]
```

### 输出

输出一段人类可读摘要，包含：

- 唤醒的目录
- `config.toml` 是新建还是保留
- `providers.toml` 是新建还是保留

### 说明

- `memo awaken [path]` 还会把该目录记录为该目录及其子目录后续命令默认使用的记忆空间
- 如需为当前进程显式覆盖活跃目标，可设置环境变量 `MEMO_DATA_DIR`

---

## `memo remember`

把一条 episode 写入 SQLite，同时更新 L0/session 状态，并把派生索引标记为 `pending`。

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
| `--dry-run` | 只预览最终 remember payload，不落库 |
| `--json` | 输出机器可读结果 |

### 说明

- `--dry-run` 会输出最终 remember payload，再决定是否落库
- 默认 `memo remember` 只会立即写入手工 entities 和 facts
- 如果用户已经显式配置 extraction provider，`--dry-run` 可以把 provider 抽取结果一起预览出来
- 默认情况下，其它命令会从当前目录向上查找最近一次 `memo awaken` 记录下来的记忆空间；`MEMO_DATA_DIR` 会覆盖它

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

- 配置了 rerank 时，deep search 可以触发精排
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
- `--json` 输出机器可读结果

---

## `memo state`

### 语法

```bash
memo state [--json]
```

### 包含内容

- episode / entity / fact / edge 数量
- layer 与 cache 状态
- 派生索引健康度
- provider 运行态健康度，包括走过降级路径时最近一次失败摘要
- 维护状态

---

## `memo restore`

在需要时基于本地真相源恢复派生层。

### 语法

```bash
memo restore [--full] [--json]
```

### 说明

- 默认执行保守恢复
- `--full` 表示从真相源完整重建派生层
- `--json` 输出机器可读结果

