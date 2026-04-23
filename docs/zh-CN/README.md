# 🧠 Memo - AI 编程助手的长期记忆

> 让 AI 记住每次对话，积累开发经验

中文 | [English](../../README.md)

---

## 💡 为什么需要 Memo？

- 💬 **AI 总是健忘** - 3 天前解释过的方案，今天又要重新讲一遍
- 🔄 **重复踩坑** - 上周修过的 bug，今天遇到类似的，AI 完全不记得
- 📚 **经验不留存** - 每次对话都是“一次性”的，宝贵经验会不断流失
- 🤝 **团队知识孤岛** - 每个人单独用 AI，无法共享经验

---

## ⚡ 核心能力

| 能力 | 说明 |
|------|------|
| 🗄️ **本地真相源** | SQLite 作为唯一真相源，保存 episodes、entities、facts、edges 与任务/索引状态 |
| 🔎 **混合检索** | 查询会组合 exact、alias、BM25、vector、graph、recency、layer、hit-frequency 等信号，并可按需进入 deep search |
| 🧩 **结构化记住** | `memo remember` 立即写入手工 entities/facts，`memo dream` 再通过 provider 慢路径补齐仍未结构化的 episode |
| 💤 **dream 工作流** | `memo dream` 负责记忆层级的晋升、冷却、归档、冲突收敛与慢路径结构化整理 |
| ♻️ **可重建索引** | text 和 vector 索引都是派生层，可以从 SQLite 真相源刷新或全量重建 |
| 🌐 **provider 扩展能力** | extraction、embedding 和 rerank 可通过 provider 配置接入 |

## 🧭 公开命令标准

公开命令语言以 [命令设计哲学](../architecture/command-philosophy.md) 为准。那个文档定义的是产品标准，不是临时说明。

系统分层、命令流程、模型边界与记忆生命周期，以 [记忆引擎架构](../architecture/memory-engine-architecture.md) 为准。

Memo 应当通过这套公开动作语言被学习和理解：

- `memo awaken`
- `memo remember`
- `memo recall`
- `memo reflect`
- `memo dream`
- `memo state`
- `memo restore`

## 🚀 快速开始

### 第一步：一键安装

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/Zoranner/memo-cli/master/install.ps1 | iex
```

**macOS/Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/Zoranner/memo-cli/master/install.sh | bash
```

引导脚本从 `master` 分支加载，随后会按当前平台自动下载最新已发布的 GitHub Release tag，并默认把 `memo` 安装到 `~/.local/bin`。如需覆盖安装目录，可设置 `MEMO_INSTALL_DIR`；如需固定版本，可将 `MEMO_VERSION` 设为显式 tag，例如 `v0.2.0`。

### 第二步：唤醒本地记忆空间

```bash
memo awaken
```

如果当前 shell 还没有拿到最新 PATH，请先重开终端再继续。

这会初始化 `~/.memo`，并把 `config.toml` 与 `providers.toml` 固定写在那里，同时准备实际使用的数据目录。
默认数据目录也是 `~/.memo`；如果想把数据文件放到别处，可通过 `MEMO_DATA_DIR` 或 `~/.memo/config.toml` 中的 `storage.data_dir` 覆盖。

### 第三步：记住并回忆

```bash
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
memo recall "Alice 住在哪里？"
memo reflect <memory-id>
```

`memo remember` 会先写入本地真相源。手工 entities 和 facts 会立即落库；`memo remember --dry-run` 仍可在不改动状态的前提下预览 provider 抽取结果。`memo recall` 负责回忆相关内容，`memo reflect` 负责查看单条记忆详情。

### 第四步：dream、restore 与 state

```bash
memo dream
memo restore
memo state
```

`memo dream` 会执行一次记忆整理；配置了 provider 时，它也会在慢路径补齐仍未结构化的 episode。`memo restore` 用于在需要时恢复派生层。`memo state` 用于查看当前引擎状态。SQLite 始终是真相源；text 和 vector 索引都是可重建的派生层。

`memo state` 会输出记录数量、layer / index 健康度，以及走过降级路径时最近一次 provider 运行态摘要。

## ⚙️ 配置说明

### 配置文件位置

- **固定配置根目录**：`~/.memo`
- **本地配置**：`~/.memo/config.toml`
- **provider 配置**：`~/.memo/providers.toml`
- **默认数据目录**：`~/.memo`

### 数据目录解析顺序

- `MEMO_DATA_DIR`
- `~/.memo/config.toml` 中的 `storage.data_dir`
- `~/.memo`

### 快速设置

1. 初始化模板：
```bash
memo awaken
```

2. 编辑 `~/.memo/providers.toml`，填入 provider 凭据

3. 编辑 `~/.memo/config.toml`，选择 provider-backed 的 extraction、embedding 或 rerank 服务

### 配置参数

| 节 | 参数 | 必填 | 说明 | 默认值 |
|----|------|:----:|------|--------|
| `[storage]` | `data_dir` | ❌ | 在保持配置文件固定于 `~/.memo` 的前提下覆盖数据目录 | `~/.memo` |
| `[embed]` | `embedding_provider` | ❌ | Embedding 服务引用，例如 `openai.embed` | - |
| `[embed]` | `duplicate_threshold` | ❌ | 重复检测阈值（0-1） | `0.85` |
| `[embed]` | `max_retries` | ❌ | 可重试 embedding 失败时的重试次数 | `0` |
| `[embed]` | `retry_backoff_ms` | ❌ | embedding 重试的线性退避基数 | `0` |
| `[extract]` | `extraction_provider` | ❌ | Extraction 服务引用，例如 `openai.extract` | - |
| `[extract]` | `min_confidence` | ❌ | 清洗后保留的最小抽取置信度 | `0.5` |
| `[extract]` | `normalize_predicates` | ❌ | 是否把抽取 predicate 归一化为稳定关系名 | `true` |
| `[extract]` | `max_retries` | ❌ | 可重试 extraction 失败时的重试次数 | `0` |
| `[extract]` | `retry_backoff_ms` | ❌ | extraction 重试的线性退避基数 | `0` |
| `[rerank]` | `rerank_provider` | ❌ | Rerank 服务引用，例如 `aliyun.rerank` | - |
| `[rerank]` | `max_retries` | ❌ | 可重试 rerank 失败时的重试次数 | `0` |
| `[rerank]` | `retry_backoff_ms` | ❌ | rerank 重试的线性退避基数 | `0` |
| `[provider.service]` | `timeout_ms` | ❌ | 单个 service 的请求超时提示 | provider 默认值 |
| `[provider.service]` | `max_concurrent` | ❌ | 透传给 provider 配置的并发提示 | provider 默认值 |

provider 引用使用 `<provider>.<service>` 形式，例如 `openai.embed` 或 `aliyun.rerank`。
`max_concurrent` 当前只负责解析并透传到 provider 配置，CLI 本身不会额外再包一层执行器级限流。

---

## 📖 更多信息

- [命令设计哲学](../architecture/command-philosophy.md) - 公开命令语言标准
- [记忆引擎架构](../architecture/memory-engine-architecture.md) - 系统分层、命令流程、模型职责与生命周期设计
- [命令参考](COMMANDS.md) - 公开命令参考
- [AI Agent Skill](../../skills/memo-brain/zh-CN/SKILL.md) - AI 编码助手集成指南
- `config.example.toml` - 主配置示例
- `providers.example.toml` - provider 配置示例
- `memo <command> --help` - 命令特定帮助

---

## 📜 License

GPL-3.0

Copyright (c) 2026 Zoranner. 保留所有权利。
