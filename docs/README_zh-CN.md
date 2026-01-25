# Memo CLI - Vector-based Knowledge Base

基于向量数据库的高性能语义搜索知识库工具。支持 **OpenAI 兼容 API**，并提供 **AI Agent Skill** 以无缝集成各类 AI 编码助手。

## ✨ 特性

- 🔍 **语义搜索** - 基于向量相似度的智能搜索，而非简单的关键词匹配
- 🤖 **AI Agent 集成** - 内置 Skill，支持 Cursor、Windsurf、Claude Code 等 AI 编码工具
- 🏷️ **标签管理** - 支持标签分类和 Markdown frontmatter 自动合并
- ⏰ **时间过滤** - 按时间范围筛选记忆，支持灵活的日期格式
- 📝 **Markdown 支持** - 自动解析和索引带 frontmatter 的 markdown 文件
- 🌐 **OpenAI 兼容** - 支持所有 OpenAI 兼容 API（OpenAI、Azure 等）
- 🏠 **本地/云端** - 支持 Ollama 本地部署和云端 API
- ⚡ **高性能** - 使用 LanceDB 向量数据库和 Rust 实现

## 📋 命令速查

| 命令 | 功能 | 示例 |
|------|------|------|
| `memo embed <input>` | 嵌入文本/文件/目录到向量数据库 | `memo embed "笔记内容" --tags rust,cli` |
| `memo search <query>` | 语义搜索记忆 | `memo search "Rust 最佳实践" --after 2026-01-20` |
| `memo list` | 列出所有记忆 | `memo list` |
| `memo clear` | 清空数据库（危险操作） | `memo clear --local --force` |
| `memo init` | 初始化配置（可选） | `memo init --local` |

**常用参数：**
- `-t, --tags` - 添加标签（逗号分隔）
- `--after / --before` - 时间范围过滤（格式：`YYYY-MM-DD` 或 `YYYY-MM-DD HH:MM`）
- `-n, --limit` - 搜索结果数量（默认：5）
- `-l, --local` - 使用本地数据库
- `-g, --global` - 使用全局数据库

## 🚀 快速开始

### 1. 安装

```bash
cargo build --release
```

### 2. 配置

创建配置文件 `~/.memo/config.toml`：

```toml
# 必填：API 密钥和模型
embedding_api_key = "your-api-key"
embedding_model = "your-model-name"

# 可选：API 端点（默认：OpenAI）
# embedding_base_url = "https://api.openai.com/v1"

# 可选：提供商类型（自动推断）
# embedding_provider = "openai"
```

### 3. 基本使用

```bash
# 嵌入文本（带标签）
memo embed "学习了 Rust 的生命周期" --tags rust,learning

# 嵌入文件
memo embed notes.md --tags important

# 嵌入目录
memo embed ./docs --tags documentation

# 搜索
memo search "Rust 最佳实践"

# 按时间范围搜索
memo search "开发经验" --after 2026-01-20 --limit 10

# 列出所有记忆
memo list
```

### 4. AI Agent 集成（可选）

适用于 **Cursor**、**Windsurf**、**Claude Code** 等 AI 编码工具：

```bash
# 将 agent skill 复制到你的 AI 工具的 skills 目录
# Cursor:
cp -r skills/memo-brain ~/.cursor/skills/

# Windsurf（示例）:
cp -r skills/memo-brain ~/.windsurf/skills/
```

安装后，你的 AI 助手可以在对话过程中自动记录和搜索记忆。详见 [AI Agent 集成](#-ai-agent-集成)章节。

## ⚙️ 配置说明

### 配置文件位置

- **全局配置**：`~/.memo/config.toml`（推荐）
- **本地配置**：`./.memo/config.toml`（项目独立）

### 配置优先级

命令行参数 > 本地配置 > 全局配置 > 默认值

### 配置参数

| 参数 | 必填 | 说明 | 默认值 |
|------|:----:|------|--------|
| `embedding_api_key` | ✅ | API 密钥 | - |
| `embedding_model` | ✅ | 模型名称 | - |
| `embedding_base_url` | ❌ | API 端点 | `https://api.openai.com/v1` |
| `embedding_provider` | ❌ | 提供商类型 | 自动推断 |
| `embedding_dimension` | ❌ | 向量维度 | 自动推断 |

### 支持的 API 类型

**OpenAI 兼容 API（默认）：**
```toml
embedding_api_key = "sk-..."
embedding_model = "text-embedding-3-small"
# embedding_base_url = "https://api.example.com/v1"  # 可选
```

**Ollama 本地部署：**
```toml
embedding_base_url = "http://localhost:11434/api"
embedding_api_key = ""  # 本地无需 key
embedding_model = "nomic-embed-text"
```

## 🤖 AI Agent 集成

Memo CLI 包含一个 **Agent Skill**（`skills/memo-brain/SKILL.md`），使 AI 编码助手能在对话过程中自动管理知识。

### 支持的 AI 编码工具

- **Cursor** - 将 skill 复制到 `~/.cursor/skills/`
- **Windsurf** - 将 skill 复制到 `~/.windsurf/skills/`
- **Claude Code** - 按工具特定方式安装 skill
- **其他 MCP 兼容工具** - 适用于支持 Agent Skills 的工具

### 核心能力

| 功能 | 说明 |
|------|------|
| **自动记录** | 自动捕获有价值的解决方案、模式和调试经验 |
| **上下文感知搜索** | 在对话过程中检索相关的过往经验 |
| **智能触发** | 识别"记住这个"或"之前是怎么解决的"等自然语言 |
| **结构化格式** | 使用一致的模板以便更好地组织和检索 |

### 安装方法

```bash
# Cursor
cp -r skills/memo-brain ~/.cursor/skills/

# Windsurf（或其他类似结构的工具）
cp -r skills/memo-brain ~/.windsurf/skills/
```

### 工作原理

安装 skill 后，你的 AI 助手会识别自然语言触发词：

**记录记忆：**
- "记住这个"
- "记录这个解决方案"
- "保存一下"

**搜索记忆：**
- "之前是怎么解决的？"
- "查一下过往记忆"
- "类似的问题我们是怎么做的？"
- "最近在...方面的工作"

**对话示例：**

```
你：  "记住这个：Rust 错误处理 - 应用层用 anyhow，库层用 thiserror"
AI：  [自动执行] memo embed "..." --tags rust,error-handling
      ✓ 已记录到知识库

你：  "之前 Rust 的 async trait 问题是怎么解决的？"
AI：  [自动执行] memo search "rust async trait" -n 5
      [基于过往经验提供答案]
```

### 手动 CLI 使用

你仍然可以不依赖 AI 集成直接使用 CLI：

```bash
# 记录结构化知识
memo embed "Rust async trait - 使用 async-trait crate

背景：trait 中直接使用 async fn 会导致编译错误
方案：在 trait 和 impl 上使用 #[async_trait] 宏
关键点：trait 定义和 impl 实现都需要添加该宏" --tags rust,async

# 搜索过往解决方案
memo search "rust async trait 问题" -n 5

# 查看最近的工作
memo search "数据库优化" --after 2026-01-20
```

查看 [skills/memo-brain/SKILL.md](../skills/memo-brain/SKILL.md) 了解详细使用指南。

## 📚 命令详解

### `memo embed` - 嵌入记忆

将文本、文件或目录嵌入向量数据库。

```bash
memo embed <input> [OPTIONS]
```

| 参数/选项 | 说明 |
|----------|------|
| `<input>` | 文本字符串、文件路径或目录路径 |
| `-t, --tags` | 添加标签（逗号分隔，如：`rust,cli`） |
| `-l, --local` | 使用本地数据库 `./.memo/brain` |
| `-g, --global` | 使用全局数据库 `~/.memo/brain` |

**示例：**
```bash
memo embed "重要笔记" --tags work,important
memo embed notes.md --tags rust,learning
memo embed ./docs --tags documentation
```

**💡 Markdown 文件标签合并：**

Markdown 文件的 frontmatter 标签会与命令行标签自动合并去重：

```markdown
---
tags: [rust, cli]
---
```

执行 `memo embed file.md --tags important` → 最终标签：`[rust, cli, important]`

---

### `memo search` - 搜索记忆

使用语义搜索查找相关记忆。

```bash
memo search <query> [OPTIONS]
```

| 参数/选项 | 说明 | 默认值 |
|----------|------|--------|
| `<query>` | 搜索查询字符串 | - |
| `-n, --limit` | 返回结果数量 | 5 |
| `-t, --threshold` | 相似度阈值（0-1） | 0.7 |
| `--after` | 时间范围：之后 | - |
| `--before` | 时间范围：之前 | - |
| `-l, --local` | 使用本地数据库 | - |
| `-g, --global` | 使用全局数据库 | - |

**时间格式：**
- `YYYY-MM-DD` - 例：`2026-01-20`（00:00）
- `YYYY-MM-DD HH:MM` - 例：`2026-01-20 14:30`

**示例：**
```bash
memo search "Rust 最佳实践"
memo search "Vue 技巧" --limit 10 --threshold 0.6
memo search "开发经验" --after 2026-01-20
memo search "会议记录" --after "2026-01-20 09:00" --before "2026-01-20 18:00"
```

---

### `memo list` - 列出记忆

列出数据库中的所有记忆（按更新时间显示）。

```bash
memo list [OPTIONS]
```

| 选项 | 说明 |
|-----|------|
| `-l, --local` | 使用本地数据库 |
| `-g, --global` | 使用全局数据库 |

---

### `memo clear` - 清空数据库

⚠️ **危险操作**：清空指定数据库中的所有记忆。

```bash
memo clear [OPTIONS]
```

| 选项 | 说明 |
|-----|------|
| `-l, --local` | 清空本地数据库 |
| `-g, --global` | 清空全局数据库 |
| `-f, --force` | 跳过确认提示（慎用） |

---

### `memo init` - 初始化配置

初始化配置（可选，首次使用会自动初始化）。

```bash
memo init [OPTIONS]
```

| 选项 | 说明 |
|-----|------|
| `-l, --local` | 在当前目录初始化本地配置 |

---

## 💡 使用技巧

### 标签策略

```bash
# 按技术栈分类
memo embed "Vue 技巧" --tags vue,frontend

# 按重要性分类
memo embed "关键决策" --tags important,decision

# 按项目分类
memo embed "项目文档" --tags project-x,docs

# 组合使用
memo embed "安全漏洞修复" --tags security,bug-fix,important
```

### 时间过滤场景

```bash
# 查看最近的记忆
memo search "开发经验" --after 2026-01-20

# 查看某段时间的工作记录
memo search "项目进展" --after 2026-01-01 --before 2026-01-31

# 查看今天的记录
memo search "会议" --after 2026-01-25
```

### 多项目管理

```bash
# 项目 A：使用本地数据库
cd /path/to/project-a
memo embed ./docs --local --tags project-a

# 项目 B：使用独立配置
cd /path/to/project-b
memo init --local  # 创建 ./.memo/config.toml
memo embed ./docs --tags project-b
```

## ❓ 常见问题

<details>
<summary><strong>如何切换不同的嵌入模型？</strong></summary>

⚠️ **重要**：不同模型的向量空间不兼容。切换模型后需要：

1. 清空数据库：`memo clear --global --force`
2. 修改配置文件中的 `embedding_model`
3. 重新嵌入所有数据

</details>

<details>
<summary><strong>本地/全局数据库有什么区别？</strong></summary>

- **全局数据库**（`~/.memo/brain`）：默认，适合个人知识库
- **本地数据库**（`./.memo/brain`）：项目独立，适合团队协作

使用 `--local` 或 `--global` 参数显式指定。

</details>

<details>
<summary><strong>Markdown 文件的标签如何处理？</strong></summary>

Markdown frontmatter 标签会与命令行标签**自动合并去重**：

```markdown
---
tags: [rust, cli]
---
```

执行 `memo embed file.md --tags important` 后：
- 最终标签：`[rust, cli, important]`

</details>

<details>
<summary><strong>时间过滤是基于创建时间还是更新时间？</strong></summary>

- 基于 **`updated_at`（更新时间）**
- 每条记忆创建时会同时记录 `created_at` 和 `updated_at`
- 时间过滤在相似度筛选**之后**进行，不影响搜索相关性

</details>

<details>
<summary><strong>如何使用 Ollama 本地部署？</strong></summary>

配置文件示例：

```toml
embedding_base_url = "http://localhost:11434/api"
embedding_api_key = ""  # 本地无需 key
embedding_model = "nomic-embed-text"
```

</details>

<details>
<summary><strong>支持哪些 OpenAI 兼容 API？</strong></summary>

支持所有遵循 OpenAI API 格式的服务，包括但不限于：
- OpenAI
- Azure OpenAI
- 各类云端 API 服务

只需配置正确的 `embedding_base_url` 和 `embedding_api_key`。

</details>

<details>
<summary><strong>支持哪些 AI 编码工具？</strong></summary>

Agent Skill 支持以下工具：
- **Cursor** - 复制 skill 到 `~/.cursor/skills/`
- **Windsurf** - 复制 skill 到 `~/.windsurf/skills/`（或工具特定位置）
- **Claude Code** - 按工具特定方式安装 skill
- **其他 MCP 兼容工具** - 查看你的工具文档了解 skill 安装路径

该 skill 设计为工具无关，遵循通用的 agent skill 模式。

</details>

<details>
<summary><strong>可以不使用 AI 集成直接用 CLI 吗？</strong></summary>

完全可以！CLI 独立运行并提供完整功能：
- **手动 CLI**：完全控制，显式命令
- **AI Agent**：自动化，对话式界面
- **组合使用**：根据需要灵活混合使用

AI Agent Skill 完全是可选的，它增加了便利性而非核心功能。

</details>

---

## 📖 更多信息

- 查看 `config.example.toml` 了解完整配置选项
- 使用 `memo <command> --help` 查看命令帮助
- 查看 `skills/memo-brain/SKILL.md` 了解 AI agent 集成详情

## 📜 License

MIT
