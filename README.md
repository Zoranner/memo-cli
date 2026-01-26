# Memo CLI - Vector Memory Store

A high-performance semantic search memory store tool powered by vector database. Supports **OpenAI-compatible APIs** and provides **AI Agent Skill** for seamless integration with AI coding assistants.

[中文](docs/zh-CN/README.md) | English

## ✨ Features

- 🔍 **Semantic Search** - Intelligent search based on vector similarity, not just keyword matching
- 🤖 **AI Agent Integration** - Built-in skill for Cursor, Windsurf, Claude Code, and other AI coding tools
- 🧠 **Smart Duplicate Detection** - Automatically detects similar memories to prevent duplicates
- 🔄 **Memory Management** - Update, delete, and merge memories for better organization
- 🏷️ **Tag Management** - Support tag classification and Markdown frontmatter auto-merge
- ⏰ **Time Filtering** - Filter memories by time range with flexible date formats
- 📝 **Markdown Support** - Auto parse and index markdown files with frontmatter
- 🌐 **OpenAI Compatible** - Support all OpenAI-compatible APIs (OpenAI, Azure, etc.)
- 🏠 **Local/Cloud** - Support Ollama local deployment and cloud APIs
- ⚡ **High Performance** - Powered by LanceDB vector database with Rust implementation

## 📋 Quick Commands

| Command | Function | Example |
|---------|----------|---------|
| `memo init` | Initialize configuration (optional) | `memo init --local` |
| `memo embed <input>` | Embed text/file/directory | `memo embed "note content" --tags rust,cli` |
| `memo search <query>` | Semantic search memories | `memo search "Rust best practices" --after 2026-01-20` |
| `memo list` | List all memories | `memo list` |
| `memo update <id>` | Update existing memory | `memo update abc123 --content "new content"` |
| `memo merge <ids>...` | Merge multiple memories | `memo merge id1 id2 id3 --content "merged"` |
| `memo delete <id>` | Delete memory | `memo delete abc123` |
| `memo clear` | Clear database (dangerous) | `memo clear --local --force` |

**Common Options:**
- `-t, --tags` - Add tags (comma-separated)
- `--after / --before` - Time range filter (format: `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`)
- `-n, --limit` - Number of search results (default: 5)
- `-l, --local` - Use local database
- `-g, --global` - Use global database

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

### 3. Basic Usage

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

### 4. AI Agent Integration (Optional)

For **Cursor**, **Windsurf**, **Claude Code**, and other AI coding tools:

```bash
# Copy the agent skill to your AI tool's skills directory
# For Cursor:
cp -r skills/memo-brain ~/.cursor/skills/

# For Windsurf (example):
cp -r skills/memo-brain ~/.windsurf/skills/
```

Once installed, your AI assistant can automatically record and search memories during conversations. See the [AI Agent Integration](#-ai-agent-integration) section for details.

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
| `similarity_threshold` | ❌ | Search similarity threshold (0-1) | `0.7` |
| `duplicate_threshold` | ❌ | Duplicate detection threshold (0-1) | `0.85` |

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

## 🤖 AI Agent Integration

Memo CLI includes an **Agent Skill** (`skills/memo-brain/SKILL.md`) that enables AI coding assistants to automatically manage knowledge during conversations.

### Supported AI Coding Tools

- **Cursor** - Copy skill to `~/.cursor/skills/`
- **Windsurf** - Copy skill to `~/.windsurf/skills/`
- **Claude Code** - Follow tool-specific skill installation
- **Any MCP-compatible tools** - Works with tools supporting Agent Skills

### Key Capabilities

| Feature | Description |
|---------|-------------|
| **Auto-Record** | Captures valuable solutions, patterns, and debugging insights automatically |
| **Context-Aware Search** | Retrieves relevant past experiences during conversations |
| **Smart Triggering** | Recognizes phrases like "remember this" or "how did we solve this before" |
| **Structured Format** | Uses consistent templates for better organization and retrieval |

### Installation

```bash
# For Cursor
cp -r skills/memo-brain ~/.cursor/skills/

# For Windsurf (or other tools with similar structure)
cp -r skills/memo-brain ~/.windsurf/skills/
```

### How It Works

Once the skill is installed, your AI assistant recognizes natural language triggers:

**Recording memories:**
- "Remember this"
- "Record this solution"
- "Save this for later"

**Searching memories:**
- "How did we solve this before?"
- "Check past memories"
- "What did we do for similar issue?"
- "Show recent work on..."

**Example conversation:**

```
You: "Remember this: Rust error handling - use anyhow for apps, thiserror for libs"
AI:  [Automatically executes] memo embed "..." --tags rust,error-handling
     ✓ Recorded to memory brain

You: "How did we handle async traits in Rust before?"
AI:  [Automatically executes] memo search "rust async trait" -n 5
     [Provides answer based on past experience]
```

### Manual CLI Usage

You can still use the CLI directly without AI integration:

```bash
# Record structured knowledge
memo embed "Rust async trait - Use async-trait crate

背景：Direct async fn in trait causes compile error
方案：Use #[async_trait] macro on trait and impl
关键点：Both trait definition and impl need the macro" --tags rust,async

# Search past solutions
memo search "rust async trait problem" -n 5

# View recent work
memo search "database optimization" --after 2026-01-20
```

See [skills/memo-brain/SKILL.md](skills/memo-brain/SKILL.md) for detailed usage guidelines.

---

## 💡 Usage Examples

> **📖 For detailed command documentation**, see [Command Reference](docs/COMMANDS.md)

### Basic Operations

```bash
# Embed text with tags
memo embed "Learned about Rust lifetimes" --tags rust,learning

# Search memories
memo search "Rust best practices" --limit 10

# List all memories
memo list

# Update a memory
memo update abc123 --content "Updated content" --tags rust,updated
```

### Advanced Usage

```bash
# Smart duplicate detection
memo embed "Similar content"  # Will check for duplicates
memo embed "Similar content" --force  # Skip duplicate check

# Time-based search
memo search "project updates" --after 2026-01-20 --before 2026-01-31

# Multi-project management
cd project-a && memo embed ./docs --local --tags project-a
cd project-b && memo init --local && memo embed ./docs --tags project-b
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

<details>
<summary><strong>Which AI coding tools are supported?</strong></summary>

The Agent Skill works with:
- **Cursor** - Copy skill to `~/.cursor/skills/`
- **Windsurf** - Copy skill to `~/.windsurf/skills/` (or tool-specific location)
- **Claude Code** - Follow tool-specific skill installation
- **Any MCP-compatible tools** - Check your tool's documentation for skill installation path

The skill is designed to be tool-agnostic and follows common agent skill patterns.

</details>

<details>
<summary><strong>Can I use the CLI without AI integration?</strong></summary>

Absolutely! The CLI works independently and provides full functionality:
- **Manual CLI**: Complete control with explicit commands
- **AI Agent**: Automated, conversational interface
- **Combined**: Mix both approaches as needed

The AI Agent Skill is entirely optional and adds convenience, not core functionality.

</details>

---

## 📖 More Information

- [Command Reference](docs/COMMANDS.md) - Detailed documentation for all commands
- [AI Agent Skill](skills/memo-brain/en-US/SKILL.md) - AI coding assistant integration guide
- `config.example.toml` - Complete configuration options
- `memo <command> --help` - Command-specific help

## 📜 License

MIT
