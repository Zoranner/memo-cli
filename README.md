# Memo CLI - Vector-based Knowledge Base

A semantic search knowledge base tool powered by vector database, supports **OpenAI-compatible APIs**.

[中文](docs/README_zh-CN.md)

## 📋 Quick Commands

| Command | Function | Example |
|---------|----------|---------|
| `memo embed <input>` | Embed text/file/directory into vector database | `memo embed "note content" --tags rust,cli` |
| `memo search <query>` | Semantic search memories | `memo search "Rust best practices" --after 2026-01-20` |
| `memo list` | List all memories | `memo list` |
| `memo clear` | Clear database (dangerous) | `memo clear --local --force` |
| `memo init` | Initialize configuration (optional) | `memo init --local` |

**Common Options:**
- `-t, --tags` - Add tags (comma-separated)
- `--after / --before` - Time range filter (format: `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`)
- `-n, --limit` - Number of search results (default: 5)
- `-l, --local` - Use local database
- `-g, --global` - Use global database

## ✨ Features

- 🔍 **Semantic Search** - Intelligent search based on vector similarity
- 🏷️ **Tag Management** - Support tag classification and Markdown frontmatter
- ⏰ **Time Filtering** - Filter memories by time range
- 📝 **Markdown Support** - Auto parse and index markdown files
- 🌐 **OpenAI Compatible** - Support all OpenAI-compatible APIs
- 🏠 **Local/Cloud** - Support Ollama local deployment and cloud APIs
- ⚡ **High Performance** - Powered by LanceDB vector database

## 🚀 Quick Start

### 1. Installation

```bash
cargo build --release
```

### 2. Configuration

Create config file `~/.memo/config.toml`:

```toml
# Required: API key and model
embedding_api_key = "your-api-key"
embedding_model = "your-model-name"

# Optional: API endpoint (default: OpenAI)
# embedding_base_url = "https://api.openai.com/v1"

# Optional: Provider type (auto-inferred)
# embedding_provider = "openai"
```

### 3. Usage

```bash
# Embed text (with tags)
memo embed "Learned about Rust lifetimes" --tags rust,learning

# Embed file
memo embed notes.md --tags important

# Embed directory
memo embed ./docs --tags documentation

# Search
memo search "Rust best practices"

# Search with time range
memo search "development experience" --after 2026-01-20 --limit 10

# List all memories
memo list
```

## ⚙️ Configuration

### Config File Locations

- **Global config**: `~/.memo/config.toml` (recommended)
- **Local config**: `./.memo/config.toml` (project-specific)

### Priority Order

Command-line args > Local config > Global config > Defaults

### Configuration Parameters

| Parameter | Required | Description | Default |
|-----------|:--------:|-------------|---------|
| `embedding_api_key` | ✅ | API key | - |
| `embedding_model` | ✅ | Model name | - |
| `embedding_base_url` | ❌ | API endpoint | `https://api.openai.com/v1` |
| `embedding_provider` | ❌ | Provider type | Auto-inferred |
| `embedding_dimension` | ❌ | Vector dimension | Auto-inferred |

### Supported API Types

**OpenAI-compatible API (default):**
```toml
embedding_api_key = "sk-..."
embedding_model = "text-embedding-3-small"
# embedding_base_url = "https://api.example.com/v1"  # Optional
```

**Ollama local deployment:**
```toml
embedding_base_url = "http://localhost:11434/api"
embedding_api_key = ""  # No key needed for local
embedding_model = "nomic-embed-text"
```

## 📚 Commands

### `memo embed` - Embed Memory

Embed text, file, or directory into vector database.

```bash
memo embed <input> [OPTIONS]
```

| Arg/Option | Description |
|------------|-------------|
| `<input>` | Text string, file path, or directory path |
| `-t, --tags` | Add tags (comma-separated, e.g., `rust,cli`) |
| `-l, --local` | Use local database `./.memo/brain` |
| `-g, --global` | Use global database `~/.memo/brain` |

**Examples:**
```bash
memo embed "Important note" --tags work,important
memo embed notes.md --tags rust,learning
memo embed ./docs --tags documentation
```

**💡 Markdown Tag Merging:**

Frontmatter tags in Markdown files are automatically merged with command-line tags:

```markdown
---
tags: [rust, cli]
---
```

Running `memo embed file.md --tags important` → Final tags: `[rust, cli, important]`

---

### `memo search` - Search Memories

Use semantic search to find relevant memories.

```bash
memo search <query> [OPTIONS]
```

| Arg/Option | Description | Default |
|------------|-------------|---------|
| `<query>` | Search query string | - |
| `-n, --limit` | Number of results | 5 |
| `-t, --threshold` | Similarity threshold (0-1) | 0.7 |
| `--after` | Time range: after | - |
| `--before` | Time range: before | - |
| `-l, --local` | Use local database | - |
| `-g, --global` | Use global database | - |

**Time Format:**
- `YYYY-MM-DD` - e.g., `2026-01-20` (00:00)
- `YYYY-MM-DD HH:MM` - e.g., `2026-01-20 14:30`

**Examples:**
```bash
memo search "Rust best practices"
memo search "Vue tips" --limit 10 --threshold 0.6
memo search "development experience" --after 2026-01-20
memo search "meeting notes" --after "2026-01-20 09:00" --before "2026-01-20 18:00"
```

---

### `memo list` - List Memories

List all memories in the database (sorted by update time).

```bash
memo list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-l, --local` | Use local database |
| `-g, --global` | Use global database |

---

### `memo clear` - Clear Database

⚠️ **Dangerous Operation**: Clear all memories in the specified database.

```bash
memo clear [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-l, --local` | Clear local database |
| `-g, --global` | Clear global database |
| `-f, --force` | Skip confirmation prompt (use with caution) |

---

### `memo init` - Initialize Configuration

Initialize configuration (optional, auto-initializes on first use).

```bash
memo init [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-l, --local` | Initialize local config in current directory |

---

## 💡 Usage Tips

### Tag Strategy

```bash
# Categorize by tech stack
memo embed "Vue tips" --tags vue,frontend

# Categorize by importance
memo embed "Critical decision" --tags important,decision

# Categorize by project
memo embed "Project docs" --tags project-x,docs

# Combine multiple categories
memo embed "Security fix" --tags security,bug-fix,important
```

### Time Filtering Scenarios

```bash
# View recent memories
memo search "development experience" --after 2026-01-20

# View work records in a time period
memo search "project progress" --after 2026-01-01 --before 2026-01-31

# View today's records
memo search "meeting" --after 2026-01-25
```

### Multi-Project Management

```bash
# Project A: Use local database
cd /path/to/project-a
memo embed ./docs --local --tags project-a

# Project B: Use separate config
cd /path/to/project-b
memo init --local  # Create ./.memo/config.toml
memo embed ./docs --tags project-b
```

## ❓ FAQ

<details>
<summary><strong>How to switch to a different embedding model?</strong></summary>

⚠️ **Important**: Vector spaces from different models are incompatible. After switching models:

1. Clear database: `memo clear --global --force`
2. Update `embedding_model` in config file
3. Re-embed all data

</details>

<details>
<summary><strong>What's the difference between local/global databases?</strong></summary>

- **Global database** (`~/.memo/brain`): Default, suitable for personal knowledge base
- **Local database** (`./.memo/brain`): Project-specific, suitable for team collaboration

Use `--local` or `--global` flag to specify explicitly.

</details>

<details>
<summary><strong>How are Markdown file tags handled?</strong></summary>

Markdown frontmatter tags are **automatically merged** with command-line tags:

```markdown
---
tags: [rust, cli]
---
```

After running `memo embed file.md --tags important`:
- Final tags: `[rust, cli, important]`

</details>

<details>
<summary><strong>Are time filters based on creation or update time?</strong></summary>

- Based on **`updated_at` (update time)**
- Both `created_at` and `updated_at` are recorded for each memory
- Time filtering happens **after** similarity filtering, doesn't affect search relevance

</details>

<details>
<summary><strong>How to use Ollama local deployment?</strong></summary>

Config file example:

```toml
embedding_base_url = "http://localhost:11434/api"
embedding_api_key = ""  # No key needed for local
embedding_model = "nomic-embed-text"
```

</details>

<details>
<summary><strong>Which OpenAI-compatible APIs are supported?</strong></summary>

All services following OpenAI API format, including but not limited to:
- OpenAI
- Azure OpenAI
- Various cloud API services

Just configure the correct `embedding_base_url` and `embedding_api_key`.

</details>

---

## 📖 More Information

- Check `config.example.toml` for complete configuration options
- Use `memo <command> --help` for command help

## 📜 License

MIT
