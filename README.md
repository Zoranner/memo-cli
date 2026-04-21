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
| 🧩 **Structured Remembering** | `memo remember` can merge raw text with manual entities/facts and optional provider extraction |
| 💤 **Consolidation Workflows** | `memo dream` promotes, cools, archives, and reconciles memory layers |
| ♻️ **Rebuildable Indexes** | Text and vector indexes are derived layers that can be refreshed or rebuilt from SQLite |
| 🌐 **Provider-Backed AI Hooks** | Extraction, embedding, and rerank can be wired through local provider configuration |

## 🧭 Public Command Standard

The public command language is defined by [Command Philosophy](docs/architecture/command-philosophy.md). That document is the product standard.

Memo should be described and learned through this public action language:

- `memo awaken`
- `memo remember`
- `memo recall`
- `memo reflect`
- `memo dream`
- `memo state`
- `memo restore`

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

### Step 2: Awaken a Local Memory Space

```bash
memo awaken
```

This creates a local `.memo` data directory with `config.toml` and `providers.toml` templates.

### Step 3: Remember and Recall

```bash
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
memo recall "Where does Alice live?"
memo reflect <memory-id>
```

`memo remember` writes memory into the local truth source. Structured entities and facts can come from manual flags and optional provider extraction. `memo recall` retrieves relevant memory, and `memo reflect` inspects one memory record in detail.

### Step 4: Dream, Restore, and Inspect State

```bash
memo dream
memo restore
memo state
```

`memo dream` runs consolidation over memory layers. `memo restore` recovers derived layers when needed. `memo state` exposes the current engine state. SQLite remains the truth source; text and vector indexes are rebuildable derived layers.

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
memo awaken
```

2. Edit `./.memo/providers.toml` with your provider credentials

3. Edit `./.memo/config.toml` to choose provider-backed extraction, embedding, or rerank services

### Configuration Parameters

| Section | Parameter | Required | Description | Default |
|---------|-----------|:--------:|-------------|---------|
| `[embed]` | `embedding_provider` | ❌ | Embedding service reference (for example `openai.embed`) | - |
| `[embed]` | `duplicate_threshold` | ❌ | Duplicate detection threshold (0-1) | `0.85` |
| `[extract]` | `extraction_provider` | ❌ | Extraction service reference (for example `openai.extract`) | - |
| `[extract]` | `min_confidence` | ❌ | Minimum extraction confidence kept after cleanup | `0.5` |
| `[extract]` | `normalize_predicates` | ❌ | Normalize extracted predicates into stable relation names | `true` |
| `[rerank]` | `rerank_provider` | ❌ | Rerank service reference (for example `aliyun.rerank`) | - |

Provider references use `<provider>.<service>` names such as `openai.embed` or `aliyun.rerank`.

---

## 📖 More Information

- [Command Philosophy](docs/architecture/command-philosophy.md) - Public command language standard
- [Command Reference](docs/COMMANDS.md) - Public command reference
- [AI Agent Skill](skills/memo-brain/en-US/SKILL.md) - AI coding assistant integration guide
- `config.example.toml` - Main configuration example
- `providers.example.toml` - Provider configuration example
- `memo <command> --help` - Command-specific help

---

## 📜 License

GPL-3.0

Copyright (c) 2026 Zoranner. All rights reserved.
