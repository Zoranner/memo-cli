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

返回 JSON，包含：

- `data_dir`
- `config_created`
- `providers_created`

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
- 结构化 entities 和 facts 可以来自手工参数，也可以来自可选 provider 抽取

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

consolidation 入口。

### 语法

```bash
memo dream [--full] [--json]
```

### 行为

- 默认执行常规 consolidation
- `--full` 代表更完整的一次 dream
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

