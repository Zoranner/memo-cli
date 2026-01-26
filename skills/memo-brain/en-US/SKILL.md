---
name: memo-brain
description: Manages contextual memories using a vector database CLI tool. Records valuable experiences, solutions, and user habits during conversations. Searches semantic memories to provide context-aware assistance. Use when needing to remember or recall information across conversations, or when the user asks to remember something or search memories.
---

# Memo Brain Management

Record and retrieve valuable knowledge using vector database semantic search.

## Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `memo embed <text>` | Record memory (text only) | `memo embed "Context:... Solution:..." --tags rust,cli` |
| `memo search <query>` | Search memories | `memo search "how to use rust async" -n 5` |
| `memo list` | List all memories | `memo list` |
| `memo update <id>` | Update memory | `memo update abc123 --content "new content"` |
| `memo merge <ids>...` | Merge memories | `memo merge id1 id2 --content "merged"` |
| `memo delete <id>` | Delete memory | `memo delete abc123` |

> **Network Access:** These commands require network access for embedding API. When using Shell tool, set `required_permissions: ["network"]` or `["all"]`.
>
> **Duplicate Detection:** When embedding, similar content will be detected with prompts. Follow the prompts as needed.

## Common Parameters

| Parameter | Purpose | Default |
|-----------|---------|---------|
| `-t, --tags` | Add tags (comma-separated) | - |
| `-n, --limit` | Result count | 5 |
| `--threshold` | Similarity threshold (0-1) | 0.7 |
| `--after` | Filter by time (YYYY-MM-DD or YYYY-MM-DD HH:MM) | - |
| `--before` | Filter by time (YYYY-MM-DD or YYYY-MM-DD HH:MM) | - |

---

## When to Record

### ✅ Record When

| Situation | Example |
|-----------|---------|
| Solved complex problem | Debugging async lifetime issues |
| Found clever solution | Non-obvious workaround or pattern |
| User says explicitly | "remember this", "record this", "save this for later" |
| Tool/library config | Setting up build tools, API config |
| User preferences | Coding style, library choices |

### ❌ Don't Record

- Simple syntax queries
- Common knowledge
- Temporary workarounds
- Duplicate content (search first)

## Content Format

**Template:**
```
[Topic] - [Short Title]

Context: [1-2 sentences describing context]
Solution: [Specific solution or knowledge]
Key points: [Key points or gotchas]
```

**Examples:**

```bash
memo embed "Rust async trait - Use async-trait crate

Context: Direct async fn in trait causes compile error
Solution: Use #[async_trait] macro on trait and impl
Key points: Both trait definition and impl need the macro" --tags rust,async
```

**Note:** Use double quotes to wrap multi-line text. On Windows CMD use `"`, on PowerShell/Bash escape inner quotes or use heredoc.

---

## When to Search

### Trigger Scenarios

| Trigger | Example Query |
|---------|---------------|
| Similar past problem | "how did we solve", "how did we do this before" |
| Recall specific solution | "how to configure", "what was that setting" |
| User asks explicitly | "search memory", "do you remember", "have we done this" |
| Start new task | Check for related experience |
| Recent work | "recently...", "what did we work on lately" (use `--after`) |

### Search Principles

Vector databases use semantic understanding to recognize the meaning and context of complete questions. For best search results:

1. **Use complete question sentences**: Describe what you want to know, not just list keywords
2. **Break down complex requests**: When facing multi-faceted questions, split into 2-3 independent sub-questions and search separately
3. **Include necessary context**: Provide enough background information to help understand intent

### Search Examples

**Simple questions:**

```bash
# Good practice: Complete question
memo search "How to use async functions in Rust traits" -n 5

# Not recommended: Keywords only
memo search "rust async trait" -n 5
```

**Complex request breakdown:**

```bash
# User request: "I want to build a Rust Web API with async requests and database connections"
# Recommended breakdown into 3 sub-questions:

memo search "How to build a Web API service in Rust" -n 5
memo search "Best practices for async request handling in Rust" -n 5
memo search "How to manage database connection pools in Rust" -n 5
```

**Parameter adjustments:**

```bash
# Get more results
memo search "How to optimize database query performance" -n 10

# Lower threshold for broader matches
memo search "What are best practices for error handling" --threshold 0.6

# Time filtering (use recent dates)
memo search "What bugs were fixed recently" --after YYYY-MM-DD

# Combined
memo search "What are common patterns for Vue components" -n 10 --after YYYY-MM-DD --threshold 0.6
```

### Parameter Guidelines

| Parameter | Typical | When to Adjust |
|-----------|---------|----------------|
| `-n` | 5 | Use 10-20 for complex topics or comprehensive understanding |
| `--threshold` | 0.7 | Lower to 0.6-0.5 if no results to get more candidates |
| `--after` | - | Find recent work or memories from specific time period |

### Using Results

1. Parse returned memories by similarity score
2. Extract relevant parts for current context
3. Cite sources: similarity score, update time, tags

---

## Best Practices

### Content Quality

| Guideline | Description |
|-----------|-------------|
| Concise | 100-300 words per memory |
| Structured | Use consistent template |
| Specific tags | Precise classification for later filtering |

### Tagging Strategy

```bash
# By technology
memo embed "..." --tags vue,frontend

# By importance  
memo embed "..." --tags important,decision

# By project
memo embed "..." --tags project-x,config

# By type
memo embed "..." --tags security,bug-fix
```

### Search Tips

| Strategy | Description |
|----------|-------------|
| Full questions first | Ask in natural language, let vector search understand complete semantics |
| Break down complex requests | Split multi-faceted questions into 2-3 sub-questions and search separately |
| Include context | Describe problem scenario and background for better matching accuracy |
| Progressive search | Start with full question, simplify or lower threshold if no results |
| Time filtering | Use `--after`/`--before` to locate memories from specific periods |

### Common Mistakes

| ❌ Don't | ✅ Do |
|----------|--------|
| Record entire code files | Extract key snippets only |
| Record every answer | Only valuable insights |
| Record without checking | Search first to avoid duplicates |
| Use vague titles | Be specific and descriptive |
| Too many generic tags | Keep tags focused |
| `memo search "rust async"` | `memo search "How to handle async in Rust traits"` |

---

## Usage Examples

### Record with Tags

```bash
memo embed "Rust error handling - Use anyhow for app-level code

Context: Application code needs simple error propagation
Solution: Use anyhow::Result as return type, ? for propagation
Key points: Use thiserror for libs, anyhow for apps" --tags rust,error-handling
```

### Memory Management

```bash
# Update memory
memo update abc123 --content "Updated content" --tags rust,updated

# Delete memory
memo delete abc123

# Merge multiple memories
memo merge id1 id2 id3 --content "Merged summary content"
```

### Time-Based Search

```bash
# Recent work (use actual dates)
memo search "database optimization experience" --after 2026-01-15

# Specific period
memo search "project progress" --after 2026-01-01 --before 2026-01-31 -n 20
```

## Trigger Phrases

| Action | Trigger Phrases |
|--------|---------|
| **Record** | "remember this", "record this", "save this", "summarize and record", "note this down" |
| **Search** | "how did we solve", "search memory", "do you remember", "recently...", "any similar experience", "have we done this before" |
