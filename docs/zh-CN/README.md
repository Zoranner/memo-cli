# 🧠 Memo - AI 编程助手的长期记忆

> 让 AI 记住每次对话，积累开发经验

中文 | [English](../../README.md)

---

## 💡 为什么需要 Memo？

- 💬 **AI 总是健忘** - 3 天前解释过的方案，今天又要重新讲一遍
- 🔄 **重复踩坑** - 上周修过的 bug，今天遇到类似的，AI 完全不记得
- 📚 **经验不留存** - 每次对话都是"一次性"的，宝贵的经验白白流失
- 🤝 **团队知识孤岛** - 每个人单独用 AI，无法共享经验

---

## ⚡ 核心能力

| 能力 | 说明 |
|------|------|
| 🤖 **自动记录** | 对话中说"记住这个"，AI 自动保存到知识库 |
| 🔍 **多维搜索** | LLM 将问题拆解为多个子问题并行搜索，最终综合生成一个完整回答 |
| ⏰ **时光回溯** | 快速找到"上周"、"上个月"的开发经验 |
| 👥 **团队协作** | 个人知识库或项目级共享，灵活选择 |
| 🔐 **数据安全** | 记忆数据完全本地存储，完全掌控 |
| 🌐 **灵活选择** | 支持 OpenAI、Ollama 本地、阿里云、智谱及任意 OpenAI 兼容服务 |

## 🚀 快速开始

### 第 1 步：一键安装

**Windows (PowerShell):**
```powershell
irm https://memo.zoran.ink/install.ps1 | iex
```

**macOS/Linux:**
```bash
curl -fsSL https://memo.zoran.ink/install.sh | bash
```

### 第 2 步：开始使用

安装后，直接在你的 AI 编程工具中使用：

```
你：数据库连接一直超时，怎么办？

AI：[自动搜索记忆] memo search "数据库连接超时" -n 5
    [未找到相关记忆]
    
    数据库超时通常是连接池配置问题...
    检查 max_connections、timeout 参数等

你：试过了，不行。后来发现是服务器防火墙挡了端口。
    要在阿里云安全组设置里开放 3306 端口。

AI：这确实是个容易忽略的问题！让我记录一下：
    
    [自动记录] memo embed "数据库连接超时 - 云服务器防火墙问题
    
    问题：数据库连接一直超时
    原因：云服务器默认不开放数据库端口
    解决：登录云控制台 → 安全组 → 添加规则 → 开放 3306 端口
    注意：还要检查服务器本地防火墙" --tags database,cloud,troubleshooting
    
    ✓ 已记录到记忆库

---

[两周后，另一个项目]

你：新项目连不上 MySQL？

AI：[自动搜索记忆] memo search "mysql connection" -n 5
    [找到相关记忆]
    
    之前遇到过类似问题！如果是云服务器，检查安全组设置：
    登录云控制台 → 安全组 → 添加规则 → 开放 3306 端口
    还要检查服务器本地防火墙设置
```

## ⚙️ 配置说明

### 配置文件位置

- **全局配置**：`~/.memo/config.toml`（推荐）
- **本地配置**：`./.memo/config.toml`（项目独立）
- **供应商配置**：`~/.memo/providers.toml`（API 密钥和服务设置）

### 配置优先级

命令行参数 > 本地配置 > 全局配置 > 默认值

### 快速设置

1. 复制示例文件：
```bash
cp providers.example.toml ~/.memo/providers.toml
cp config.example.toml ~/.memo/config.toml
```

2. 编辑 `~/.memo/providers.toml` 填入你的 API 密钥

3. 编辑 `~/.memo/config.toml` 选择要使用的服务

### 配置参数

| 节 | 参数 | 必填 | 说明 | 默认值 |
|----|------|:----:|------|--------|
| `[embed]` | `embedding_provider` | ✅ | Embedding 服务引用（如 `aliyun.embed`） | - |
| `[embed]` | `duplicate_threshold` | ❌ | 重复检测相似度阈值（0-1） | `0.85` |
| `[search]` | `rerank_provider` | ✅ | Rerank 服务引用（如 `aliyun.rerank`） | - |
| `[search]` | `llm_provider` | ✅ | 默认 LLM，用于拆解和总结（如 `aliyun.llm`） | - |
| `[search]` | `results_limit` | ❌ | 搜索结果数量上限 | `10` |
| `[search]` | `similarity_threshold` | ❌ | 向量搜索相似度阈值（0-1） | `0.35` |
| `[decompose]` | `llm_provider` | ❌ | 拆解专用 LLM（覆盖 `search.llm_provider`） | - |
| `[decompose]` | `max_queries` | ❌ | 最大子查询数 | `12` |
| `[decompose]` | `strategy_prompt` | ❌ | 自定义拆解策略提示词 | 内置五维策略 |
| `[merge]` | `candidates_per_query` | ❌ | 每个子查询召回候选数 | `50` |
| `[merge]` | `results_per_query` | ❌ | 合并前每个子查询保留结果数 | `5` |
| `[merge]` | `max_results` | ❌ | 合并后最终结果数上限 | `20` |
| `[merge]` | `dedup_threshold` | ❌ | 去重阈值（0-1） | `0.98` |
| `[summarize]` | `llm_provider` | ❌ | 总结专用 LLM（覆盖 `search.llm_provider`） | - |
| `[summarize]` | `strategy_prompt` | ❌ | 自定义总结策略提示词 | 内置策略 |

---

## 📖 更多信息

- [命令参考](COMMANDS.md) - 所有命令的详细文档
- [AI Agent Skill](../../skills/memo-brain/zh-CN/SKILL.md) - AI 编码助手集成指南
- `config.example.toml` - 主配置示例
- `providers.example.toml` - 供应商配置示例
- `memo <command> --help` - 命令特定帮助

---

## 📜 License

GPL-3.0

Copyright (c) 2026 Zoranner. 保留所有权利。
