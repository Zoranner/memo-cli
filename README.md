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
| 🧩 **Structured Remembering** | `memo remember` writes manual entities/facts immediately, and `memo dream` can enrich unstructured episodes through provider-backed extraction |
| 💤 **Dream Workflows** | `memo dream` promotes, cools, archives, reconciles memory layers, and performs slow-path structural consolidation |
| ♻️ **Rebuildable Indexes** | Text and vector indexes are derived layers that can be refreshed or rebuilt from SQLite |
| 🌐 **Provider-Backed AI Hooks** | Extraction, embedding, and rerank can be wired through provider configuration |

## 🧭 Public Command Standard

The public command language is defined by [Command Philosophy](docs/architecture/command-philosophy.md). That document is the product standard.

The system architecture, runtime flow, model boundaries, and memory lifecycle are defined by [Memory Engine Architecture](docs/architecture/memory-engine-architecture.md).

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
irm https://raw.githubusercontent.com/Zoranner/memo-cli/master/install.ps1 | iex
```

**macOS/Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/Zoranner/memo-cli/master/install.sh | bash
```

The bootstrap script is loaded from the `master` branch, then downloads the latest published GitHub Release tag for your platform and installs `memo` into `~/.local/bin` by default. Override the destination with `MEMO_INSTALL_DIR` or pin an explicit release tag such as `v0.2.0` with `MEMO_VERSION`.

### Step 2: Awaken a Local Memory Space

```bash
memo awaken
```

If your current shell has not picked up the updated `PATH` yet, restart it first.

This initializes `~/.memo`, keeps `config.toml` and `providers.toml` there, and prepares the active data directory.
By default the data directory is also `~/.memo`. Set `MEMO_DATA_DIR` or `storage.data_dir` in `~/.memo/config.toml` when you need to move the data files elsewhere.

### Step 3: Remember and Recall

```bash
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
memo recall "Where does Alice live?"
memo reflect <memory-id>
```

`memo remember` writes memory into the local truth source. Manual entities and facts are written immediately; `memo remember --dry-run` can still preview provider-backed extraction without mutating state. `memo recall` retrieves relevant memory, and `memo reflect` inspects one memory record in detail.

### Step 4: Dream, Restore, and Inspect State

```bash
memo dream
memo restore
memo state
```

`memo dream` runs a dream pass over memory layers, including slow-path provider extraction for still-unstructured episodes when configured. `memo restore` recovers derived layers when needed. `memo state` exposes the current engine state. SQLite remains the truth source; text and vector indexes are rebuildable derived layers.

`memo state` reports record counts, layer/index health, and the latest provider runtime degradation summary when fallback paths were used.

## ⚙️ Configuration

### Config File Locations

- **Fixed config root**: `~/.memo`
- **Local config**: `~/.memo/config.toml`
- **Providers config**: `~/.memo/providers.toml`
- **Default data dir**: `~/.memo`

### Data Dir Resolution

- `MEMO_DATA_DIR`
- `storage.data_dir` from `~/.memo/config.toml`
- `~/.memo`

### Quick Setup

1. Initialize templates:
```bash
memo awaken
```

2. Edit `~/.memo/providers.toml` with your provider credentials

3. Edit `~/.memo/config.toml` to choose provider-backed extraction, embedding, or rerank services

### Configuration Parameters

| Section | Parameter | Required | Description | Default |
|---------|-----------|:--------:|-------------|---------|
| `[storage]` | `data_dir` | ❌ | Override the data directory while keeping config files under `~/.memo` | `~/.memo` |
| `[embed]` | `embedding_provider` | ❌ | Embedding service reference (for example `openai.embed`) | - |
| `[embed]` | `duplicate_threshold` | ❌ | Duplicate detection threshold (0-1) | `0.85` |
| `[embed]` | `max_retries` | ❌ | Retry count for retryable embedding failures | `0` |
| `[embed]` | `retry_backoff_ms` | ❌ | Linear backoff base for embedding retries | `0` |
| `[extract]` | `extraction_provider` | ❌ | Extraction service reference (for example `openai.extract`) | - |
| `[extract]` | `min_confidence` | ❌ | Minimum extraction confidence kept after cleanup | `0.5` |
| `[extract]` | `normalize_predicates` | ❌ | Normalize extracted predicates into stable relation names | `true` |
| `[extract]` | `max_retries` | ❌ | Retry count for retryable extraction failures | `0` |
| `[extract]` | `retry_backoff_ms` | ❌ | Linear backoff base for extraction retries | `0` |
| `[rerank]` | `rerank_provider` | ❌ | Rerank service reference (for example `aliyun.rerank`) | - |
| `[rerank]` | `max_retries` | ❌ | Retry count for retryable rerank failures | `0` |
| `[rerank]` | `retry_backoff_ms` | ❌ | Linear backoff base for rerank retries | `0` |
| `[provider.service]` | `timeout_ms` | ❌ | Per-service request timeout hint | provider default |
| `[provider.service]` | `max_concurrent` | ❌ | Per-service concurrency hint forwarded into provider config | provider default |

Provider references use `<provider>.<service>` names such as `openai.embed` or `aliyun.rerank`.
`max_concurrent` is currently parsed and forwarded into provider config, but the CLI does not add an extra executor-level limiter on top of the provider implementation.

---

## 📖 More Information

- [Command Philosophy](docs/architecture/command-philosophy.md) - Public command language standard
- [Memory Engine Architecture](docs/architecture/memory-engine-architecture.md) - System layers, command flows, model boundaries, and lifecycle design
- [Command Reference](docs/COMMANDS.md) - Public command reference
- [AI Agent Skill](skills/memo-brain/en-US/SKILL.md) - AI coding assistant integration guide
- `config.example.toml` - Main configuration example
- `providers.example.toml` - Provider configuration example
- `memo <command> --help` - Command-specific help

---

## 📜 License

GPL-3.0

Copyright (c) 2026 Zoranner. All rights reserved.
