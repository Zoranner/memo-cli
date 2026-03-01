# Command Reference

Detailed documentation for all Memo CLI commands.

[中文](zh-CN/COMMANDS.md) | English

## Table of Contents

- [`memo init`](#memo-init---initialize-configuration) - Initialize configuration
- [`memo embed`](#memo-embed---embed-memory) - Embed text/file/directory
- [`memo search`](#memo-search---search-memories) - Semantic search
- [`memo list`](#memo-list---list-memories) - List all memories
- [`memo update`](#memo-update---update-memory) - Update existing memory
- [`memo merge`](#memo-merge---merge-memories) - Merge multiple memories
- [`memo delete`](#memo-delete---delete-memory) - Delete memory
- [`memo clear`](#memo-clear---clear-database) - Clear database

---

## `memo init` - Initialize Configuration

Initialize configuration (optional, auto-initializes on first use).

Creates the following files:
- `~/.memo/config.toml` - Main application configuration
- `~/.memo/providers.toml` - API provider configuration (you need to create this from example)

### Syntax

```bash
memo init [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `-l, --local` | Initialize local config in current directory |

### Examples

```bash
# Initialize global config (default)
memo init

# Initialize local config for current project
memo init --local
```

### Setup Guide

After `memo init`, you need to configure providers:

```bash
# Copy the providers example
cp providers.example.toml ~/.memo/providers.toml

# Edit providers.toml with your API keys
#    - Add your Alibaba Cloud / Zhipu AI / OpenAI API keys
#    - Configure services (embedding, rerank, llm)

# Edit config.toml to select services
#    embedding = "aliyun.embed"
#    rerank = "aliyun.rerank"
#    llm = "aliyun.llm"
```

See `providers.example.toml` and `config.example.toml` for complete examples.

---

## `memo embed` - Embed Memory

Embed text, file, or directory into vector database.

**Smart Duplicate Detection**: By default, `embed` checks for similar memories and cancels the operation if duplicates are found.

### Syntax

```bash
memo embed <input> [OPTIONS]
```

### Arguments & Options

| Arg/Option | Description | Default |
|------------|-------------|---------|
| `<input>` | Text string, file path, or directory path | - |
| `-t, --tags` | Add tags (comma-separated, e.g., `rust,cli`) | - |
| `-f, --force` | Skip duplicate check and force add | `false` |
| `--dup-threshold` | Similarity threshold for duplicate detection (0-1, overrides config) | `0.85` |
| `-l, --local` | Use local database `./.memo/brain` | - |
| `-g, --global` | Use global database `~/.memo/brain` | - |

### Examples

```bash
# Embed text with tags
memo embed "Important note" --tags work,important

# Force add without duplicate check
memo embed "Similar but different content" --force

# Custom duplicate threshold
memo embed "Content" --dup-threshold 0.9

# Embed files and directories
memo embed notes.md --tags rust,learning
memo embed ./docs --tags documentation
```

### Duplicate Detection Workflow

When similar memories are detected:

```
    Database ~/.memo/brain (16 records)

    Encoding text
    Checking for similar memories

     Warning Found 2 similar memories (threshold: 0.85)

[0.92] abc123-def456-789abc... (2026-01-20 10:30)
       Rust async trait - Use async-trait crate
       Background: Direct async fn in trait causes compile error

[0.88] def456-789abc-012def... (2026-01-19 15:20)
       Another related async pattern...

        Note Use --force to add anyway, or update/merge/delete existing memories
```

**Suggested actions:**
- Force add: `memo embed "..." --force`
- Update: `memo update <id> --content "..."`
- Merge: `memo merge <id1> <id2> --content "..."`
- Delete: `memo delete <id>`

### Markdown Tag Merging

Frontmatter tags in Markdown files are automatically merged with command-line tags:

```markdown
---
tags: [rust, cli]
---
```

Running `memo embed file.md --tags important` → Final tags: `[rust, cli, important]`

---

## `memo search` - Search Memories

Search and explore related memories using semantic similarity.

### Syntax

```bash
memo search <query> [OPTIONS]
```

### Arguments & Options

| Arg/Option | Description | Default |
|------------|-------------|---------|
| `<query>` | Search query string | - |
| `-n, --limit` | Maximum results to return | 10 |
| `-t, --threshold` | Similarity threshold (0-1) | 0.35 |
| `--after` | Time range: after | - |
| `--before` | Time range: before | - |
| `-l, --local` | Use local database | - |
| `-g, --global` | Use global database | - |

### Time Format

- `YYYY-MM-DD` - e.g., `2026-01-20` (00:00)
- `YYYY-MM-DD HH:MM` - e.g., `2026-01-20 14:30`

### How It Works

Search uses a multi-query pipeline powered by LLM:

1. **Query Decomposition**
   - LLM breaks the query into multiple independent sub-questions (BFS, up to `max_level` levels, up to `max_total_leaves` leaf nodes)
   - Default strategy: five-dimensional model (core, why, how, case, note) — customizable via `[prompts].decompose`

2. **Parallel Sub-Query Search**
   - Each leaf sub-question is searched in parallel using multi-layer vector search
   - Layer 1 uses the configured threshold; deeper layers use progressively higher thresholds to find related memories
   - Up to `candidates_per_query` candidates are fetched per sub-query, then reranked to keep `top_n_per_leaf`

3. **Merge & Deduplication**
   - Results from all sub-queries are merged by memory ID (highest score wins)
   - Each sub-query is guaranteed at least `min_per_leaf` results in the final output
   - Total results are capped at `min(--limit, max_total_results)`

4. **LLM Summarization**
   - The merged results are sent to LLM along with the original query
   - LLM synthesizes a comprehensive answer based on relevant memories
   - Summary strategy is customizable via `[prompts].summarize`

**Intelligent Reranking** (per sub-query):

- **Rerank Skipped** when candidates ≤ limit, or average similarity is very high
- **Rerank Applied** for larger or mixed-quality candidate sets

**Score Types:**
- `V:` prefix = Vector similarity score (from embedding model)
- `R:` prefix = Rerank score (from rerank model, more accurate)

**Time Filtering:**
- Use `--after` and `--before` to filter by date range
- Supports flexible date formats: `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`

### Examples

```bash
# Basic search
memo search "Rust best practices"

# Search with custom parameters
memo search "Vue tips" --limit 10 --threshold 0.6

# Time-based search
memo search "development experience" --after 2026-01-20
memo search "meeting notes" --after "2026-01-20 09:00" --before "2026-01-20 18:00"

# Search with more results
memo search "async patterns" -n 20
memo search "error handling" --threshold 0.65 -n 30
```

### Output Example

Search first displays the LLM-synthesized answer, then the source memories with relevance scores:

```
  Decomposing query into sub-questions (max_level=3)
   Decomposed 5 sub-questions
    Searching 5 sub-queries in parallel
      Merging results from 5 sub-queries
      Results 3 results from multi-query search
  Summarizing results with LLM

For async trait patterns in Rust, use the `async-trait` crate. Add `#[async_trait]`
to both the trait definition and all implementations. This is because Rust's type
system cannot represent the return type of async fn in traits directly...

[R:0.89] a1b2c3d4-e5f6-7890-abcd-ef1234567890 (2026-01-27 10:30) [rust, async, trait]
         Rust async patterns - async-trait usage guide
         
         Context: Using async fn directly in traits causes compilation errors
         Solution: Use #[async_trait] macro on trait definitions and implementations

[R:0.85] b2c3d4e5-f6a7-8901-bcde-f12345678901 (2026-01-26 14:20) [rust, async, error]
         Async error handling - Result<T, E> usage

[V:0.82] f9a8b7c6-d5e4-3210-fedc-ba9876543210 (2026-01-26 15:45) [rust, error]
         Rust error handling best practices
```

**Score Prefixes:**
- `R:` = Rerank score (more accurate, semantically re-ordered)
- `V:` = Vector similarity score (from embedding model)

---

## `memo list` - List Memories

List all memories in the database (sorted by update time).

### Syntax

```bash
memo list [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `-l, --local` | Use local database |
| `-g, --global` | Use global database |

---

## `memo update` - Update Memory

Update an existing memory's content and tags.

### Syntax

```bash
memo update <id> [OPTIONS]
```

### Arguments & Options

| Arg/Option | Description |
|------------|-------------|
| `<id>` | Memory ID to update |
| `-c, --content` | New content (required) |
| `-t, --tags` | New tags (comma-separated, replaces existing tags) |
| `-l, --local` | Use local database |
| `-g, --global` | Use global database |

### Examples

```bash
# Update content only
memo update abc123 --content "Updated content"

# Update both content and tags
memo update abc123 --content "New content" --tags rust,updated,important
```

**Note:** Updating regenerates the embedding vector while preserving the original `created_at` timestamp.

---

## `memo merge` - Merge Memories

Merge multiple memories into a single consolidated memory.

### Syntax

```bash
memo merge <ids>... [OPTIONS]
```

### Arguments & Options

| Arg/Option | Description |
|------------|-------------|
| `<ids>...` | Memory IDs to merge (space-separated) |
| `-c, --content` | Content for merged memory (required) |
| `-t, --tags` | Tags for merged memory (auto-merges all tags if not specified) |
| `-l, --local` | Use local database |
| `-g, --global` | Use global database |

### Examples

```bash
# Merge with custom content
memo merge id1 id2 id3 --content "Consolidated knowledge about..."

# Merge with custom content and tags
memo merge id1 id2 --content "Merged content" --tags rust,summary

# Merge (tags auto-merged from all memories)
memo merge id1 id2 id3 --content "Combined insights"
```

**Note:** The merged memory preserves the earliest `created_at` timestamp from the original memories.

---

## `memo delete` - Delete Memory

Delete a memory by ID.

### Syntax

```bash
memo delete <id> [OPTIONS]
```

### Arguments & Options

| Arg/Option | Description |
|------------|-------------|
| `<id>` | Memory ID to delete |
| `-f, --force` | Skip confirmation prompt |
| `-l, --local` | Use local database |
| `-g, --global` | Use global database |

### Examples

```bash
memo delete abc123
memo delete abc123 --force
memo delete xyz789 --local
```

**Note:** You'll be prompted to confirm by typing `yes` unless `--force` is specified.

---

## `memo clear` - Clear Database

⚠️ **Dangerous Operation**: Clear all memories in the specified database.

### Syntax

```bash
memo clear [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `-l, --local` | Clear local database |
| `-g, --global` | Clear global database |
| `-f, --force` | Skip confirmation prompt (use with caution) |

---

## Common Options

These options are available across multiple commands:

| Option | Description |
|--------|-------------|
| `-l, --local` | Use local database (`./.memo/brain`) |
| `-g, --global` | Use global database (`~/.memo/brain`) |
| `-t, --tags` | Add/set tags (comma-separated) |
| `-f, --force` | Skip confirmation prompts |

## Usage Tips

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
