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

Use semantic search to find relevant memories.

### Syntax

```bash
memo search <query> [OPTIONS]
```

### Arguments & Options

| Arg/Option | Description | Default |
|------------|-------------|---------|
| `<query>` | Search query string | - |
| `-n, --limit` | Number of results | 5 |
| `-t, --threshold` | Similarity threshold (0-1) | 0.7 |
| `--after` | Time range: after | - |
| `--before` | Time range: before | - |
| `-l, --local` | Use local database | - |
| `-g, --global` | Use global database | - |

### Time Format

- `YYYY-MM-DD` - e.g., `2026-01-20` (00:00)
- `YYYY-MM-DD HH:MM` - e.g., `2026-01-20 14:30`

### Examples

```bash
memo search "Rust best practices"
memo search "Vue tips" --limit 10 --threshold 0.6
memo search "development experience" --after 2026-01-20
memo search "meeting notes" --after "2026-01-20 09:00" --before "2026-01-20 18:00"
```

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
