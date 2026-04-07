# 🧠 Memo - Long-Term Memory for Your AI Coding Assistant

> Help AI remember every conversation and accumulate development experience

[中文](docs/zh-CN/README.md) | English

---

## 💡 Why Memo?

- 💬 **AI keeps forgetting** - Explained a solution 3 days ago, have to explain again today
- 🔄 **Solving same problems** - Fixed a bug last week, similar one today, AI doesn't remember
- 📚 **Knowledge doesn't stick** - Every conversation is "one-off", valuable experience lost
- 🤝 **Team knowledge silos** - Everyone uses AI separately, can't share experience

---

## ⚡ Core Capabilities

| Capability | Description |
|------------|-------------|
| 🤖 **Auto Recording** | Say "remember this" in conversation, AI automatically saves to knowledge base |
| 🔍 **Multi-Query Search** | LLM decomposes your question into sub-queries, searches in parallel, and synthesizes a comprehensive answer |
| ⏰ **Time Travel** | Quickly find development experience from "last week" or "last month" |
| 👥 **Team Collaboration** | Personal knowledge base or project-level sharing, flexible choice |
| 🔐 **Data Security** | Memory data stored entirely locally, fully under your control |
| 🌐 **Flexible Options** | Supports OpenAI, Ollama local, Aliyun, ZhipuAI, and any OpenAI-compatible service |

## 🚀 Quick Start

### Step 1: One-Click Install

**Windows (PowerShell):**
```powershell
irm https://memo.zoran.ink/install.ps1 | iex
```

**macOS/Linux:**
```bash
curl -fsSL https://memo.zoran.ink/install.sh | bash
```

### Step 2: Start Using

After installation, start using it directly in your AI coding tool:

```
You: Database connection keeps timing out, how to fix it?

AI: [Auto searches memory] memo search "database connection timeout" -n 5
    [No memories found]
    
    Database timeouts are usually caused by connection pool settings...
    Check max_connections, timeout parameters, etc.

You: Tried that, didn't work. Turns out the server firewall was blocking the port.
    Had to open port 3306 in Alibaba Cloud security group settings.

AI: Ah, that's an easy-to-miss issue! Let me record this:
    
    [Auto records] memo embed "Database Connection Timeout - Cloud Firewall Issue
    
    Problem: Database connection keeps timing out
    Cause: Cloud server doesn't open database port by default
    Solution: Login to cloud console → Security groups → Add rule → Open port 3306
    Note: Also check server's local firewall" --tags database,cloud,troubleshooting
    
    ✓ Recorded to memory

---

[Two weeks later, different project]

You: Can't connect to MySQL on the new project?

AI: [Auto searches memory] memo search "mysql connection" -n 5
    [Memory found]
    
    Had a similar issue before! If it's a cloud server, check security group settings:
    Login to cloud console → Security groups → Add rule → Open port 3306
    Also check the server's local firewall settings
```

## ⚙️ Configuration

### Config File Locations

- **Global config**: `~/.memo/config.toml` (recommended)
- **Local config**: `./.memo/config.toml` (project-specific)
- **Providers config**: `~/.memo/providers.toml` (API keys and service settings)

### Priority Order

Command-line args > Local config > Global config > Defaults

### Quick Setup

1. Copy example files:
```bash
cp providers.example.toml ~/.memo/providers.toml
cp config.example.toml ~/.memo/config.toml
```

2. Edit `~/.memo/providers.toml` with your API keys

3. Edit `~/.memo/config.toml` to select your preferred services

### Configuration Parameters

| Section | Parameter | Required | Description | Default |
|---------|-----------|:--------:|-------------|---------|
| `[embed]` | `embedding_provider` | ✅ | Embedding service reference (e.g., `aliyun.embed`) | - |
| `[embed]` | `duplicate_threshold` | ❌ | Duplicate detection threshold (0-1) | `0.85` |
| `[search]` | `rerank_provider` | ✅ | Rerank service reference (e.g., `aliyun.rerank`) | - |
| `[search]` | `llm_provider` | ✅ | Default LLM for decompose & summarize (e.g., `aliyun.llm`) | - |
| `[search]` | `results_limit` | ❌ | Maximum search results | `10` |
| `[search]` | `similarity_threshold` | ❌ | Vector search similarity threshold (0-1) | `0.35` |
| `[decompose]` | `llm_provider` | ❌ | LLM override for decompose (overrides `search.llm_provider`) | - |
| `[decompose]` | `max_queries` | ❌ | Maximum number of sub-queries | `12` |
| `[decompose]` | `strategy_prompt` | ❌ | Custom decompose strategy prompt | built-in 5D strategy |
| `[merge]` | `candidates_per_query` | ❌ | Candidates retrieved per sub-query | `50` |
| `[merge]` | `results_per_query` | ❌ | Results kept per sub-query before merge | `5` |
| `[merge]` | `max_results` | ❌ | Maximum final results after merge | `20` |
| `[merge]` | `dedup_threshold` | ❌ | Deduplication threshold (0-1) | `0.98` |
| `[summarize]` | `llm_provider` | ❌ | LLM override for summarize (overrides `search.llm_provider`) | - |
| `[summarize]` | `strategy_prompt` | ❌ | Custom summarize strategy prompt | built-in strategy |

---

## 📖 More Information

- [Command Reference](docs/COMMANDS.md) - Detailed documentation for all commands
- [AI Agent Skill](skills/memo-brain/en-US/SKILL.md) - AI coding assistant integration guide
- `config.example.toml` - Main configuration example
- `providers.example.toml` - Provider configuration example
- `memo <command> --help` - Command-specific help

---

## 📜 License

GPL-3.0

Copyright (c) 2026 Zoranner. All rights reserved.
