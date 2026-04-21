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
| 🗄️ **Local Truth Source** | SQLite stores episodes, entities, facts, edges, and job/index state as the single source of truth |
| 🔎 **Hybrid Retrieval** | Queries combine exact, alias, BM25, vector, graph, recency, layer, and hit-frequency signals with optional deep search |
| 🧩 **Structured Ingest** | `memo ingest` can merge raw text with manual entities/facts and optional provider extraction |
| 💤 **Consolidation Workflows** | `memo dream` and `memo run-dream-jobs` promote, cool, archive, and reconcile memory layers |
| ♻️ **Rebuildable Indexes** | Text and vector indexes are derived layers that can be refreshed or rebuilt from SQLite |
| 🌐 **Provider-Backed AI Hooks** | Extraction, embedding, and rerank can be wired through local provider configuration |

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

### Step 2: Initialize a Local Data Directory

```bash
memo init
```

This creates a local `.memo` data directory with `config.toml` and `providers.toml` templates.

### Step 3: Ingest and Query Memory

```bash
memo ingest "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
memo query "Where does Alice live?"
memo inspect <memory-id>
```

`memo ingest` writes to SQLite first. Structured entities and facts can come from manual flags and optional provider extraction.

### Step 4: Run Maintenance Workflows

```bash
memo dream --trigger manual
memo refresh-index --scope all
memo stats
```

`memo dream` runs consolidation over memory layers. `memo refresh-index` updates any derived indexes currently marked as pending. SQLite remains the truth source; text and vector indexes are rebuildable derived layers.

## ⚙️ Configuration

### Config File Locations

- **Default local data dir**: `./.memo`
- **Local config**: `./.memo/config.toml`
- **Providers config**: `./.memo/providers.toml`

### Priority Order

Command-line args > Local config > Defaults

### Quick Setup

1. Initialize templates:
```bash
memo init
```

2. Edit `./.memo/providers.toml` with your provider credentials

3. Edit `./.memo/config.toml` to choose provider-backed extraction, embedding, or rerank services

### Configuration Parameters

| Section | Parameter | Required | Description | Default |
|---------|-----------|:--------:|-------------|---------|
| `[embed]` | `embedding_provider` | ❌ | Embedding service reference (for example `openai.embed`) | - |
| `[embed]` | `duplicate_threshold` | ❌ | Duplicate detection threshold (0-1) | `0.85` |
| `[extract]` | `extraction_provider` | ❌ | Extraction service reference (for example `openai.llm`) | - |
| `[extract]` | `min_confidence` | ❌ | Minimum extraction confidence kept after cleanup | `0.5` |
| `[extract]` | `normalize_predicates` | ❌ | Normalize extracted predicates into stable relation names | `true` |
| `[rerank]` | `rerank_provider` | ❌ | Rerank service reference (for example `aliyun.rerank`) | - |

Provider references use `<provider>.<service>` names such as `openai.embed` or `aliyun.rerank`.

---

## 📖 More Information

- [Command Reference](docs/COMMANDS.md) - Detailed documentation for all current CLI commands
- [AI Agent Skill](skills/memo-brain/en-US/SKILL.md) - AI coding assistant integration guide
- `config.example.toml` - Main configuration example
- `providers.example.toml` - Provider configuration example
- `memo <command> --help` - Command-specific help

---

## 📜 License

GPL-3.0

Copyright (c) 2026 Zoranner. All rights reserved.
