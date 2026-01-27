---
name: memo-brain
description: Manages and retrieves contextual memories across conversations using a vector database. Records experiences, solutions, and user habits. Provides context-aware assistance through semantic search. Use when needing to remember information, search memories, or when the user explicitly asks to "remember this", "search memory", or "do you remember".
---

# Memo Brain Management

Record and retrieve valuable knowledge using vector database semantic search.

**⚠️ Important: All commands require network access (for embedding API). When using Shell tool, set `required_permissions: ["network"]` or `["all"]`.**

## Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `memo embed <text>` | Record memory (text only) | `memo embed "Context:... Solution:..." --tags rust,cli` |
| `memo search <query>` | Search memories | `memo search "how to use rust async" -n 5` |
| `memo list` | List all memories | `memo list` |
| `memo update <id>` | Update memory | `memo update abc123 --content "new content" --tags rust,async` |
| `memo merge <ids>...` | Merge memories | `memo merge id1 id2 --content "merged" --tags rust,cli` |
| `memo delete <id>` | Delete memory | `memo delete abc123 --force` |

**Duplicate Detection:** When embedding detects similar content (similarity > 0.85), prioritize merging or updating instead of blindly creating new memories.

## Common Parameters

| Parameter | Purpose | Default |
|-----------|---------|---------|
| `-t, --tags` | Add tags (comma-separated) | - |
| `-n, --limit` | Search result count | 5 |
| `--threshold` | Similarity threshold (0-1) | 0.7 |
| `--after / --before` | Time filter (YYYY-MM-DD) | - |

---

## Core Workflows

### When to Record

**Record when:**
- Solved complex problem or found clever solution
- User explicitly asks ("remember this", "record this")
- Tool configurations, user preferences, or valuable experiences

**Don't record:**
- Simple syntax, common knowledge, or temporary workarounds
- Duplicate content (**search existing memories first**, prioritize merge/update)

### When to Search

**Trigger scenarios:**
- Similar problem or task ("how did we solve this before")
- User explicitly asks ("search memory", "do you remember")
- Before starting new tasks, check for related experience
- Find recent work (use `--after`)

**Search principles:**
- ✅ Use complete, specific question sentences with detailed information (tech stack, scenario, problem)
- ❌ Don't just list 2-3 keywords (e.g., "rust async trait")
- Break complex requests into 2-3 sub-questions and search separately
- More detail is better: specific tech stack, specific use case, specific problem description

### Handling Duplicate Memories

When `memo embed` detects similar memories (similarity > 0.85), the system automatically displays complete information about similar memories (ID, content, score, date). **Don't skip or force add immediately**. Evaluate based on the displayed information and decide:

**Decision workflow:**
```bash
# 1. Review the similar memory information displayed by the system (already includes complete content)

# 2. Evaluate and decide:
#    - New content refines/supplements existing → use update
#    - Multiple memories overlap and can be consolidated → use merge

# Example A: Update existing memory (add details)
memo update abc123 --content "Original content + new details and supplements" --tags rust,async

# Example B: Merge similar memories (consolidate overlapping content)
memo merge id1 id2 --content "Consolidated content covering key points from both memories" --tags rust,error-handling

# Example C: Delete and re-embed (complete replacement)
memo delete abc123 --force
memo embed "Completely new content..." --tags rust,optimization
```

**Decision Priority:**
1. ✅ **Merge** - Content overlaps and can be consolidated into one clear, complete memory (**mind granularity, don't merge blindly**)
2. ✅ **Update** - New content supplements and refines existing memory (**avoid making memory too large**)
3. ✅ **Split** - Existing memory is too large or contains multiple independent topics (**keep each memory focused on single topic**)
4. ✅ **Add new** - Confirmed as completely independent new knowledge

**Memory Granularity Control Principles:**
- ✅ **Right size**: Each memory 100-300 words, not exceeding 500 words
- ✅ **Single topic**: Each memory focuses on one core topic or problem
- ✅ **Clear structure**: Context, solution, key points are distinct and easy to understand quickly
- ❌ **Over-merging**: Don't force together different scenarios or problems
- ❌ **Over-fragmentation**: Don't split closely related content too finely

**When to split memories:**
- Single memory exceeds 500 words with multiple independent points
- Memory covers multiple different scenarios or problems
- Memory mixes multiple tech stacks or concepts
- Search often only needs a specific part of the content

---

## Content Format

**Template:**
```
[Topic] - [Short Title]

Context: [1-2 sentences describing context]
Solution: [Specific solution or knowledge]
Key points: [Key points or gotchas]
```

**Example:**

```bash
memo embed "Rust async trait - Use async-trait crate

Context: Direct async fn in trait causes compile error
Solution: Use #[async_trait] macro on trait and impl
Key points: Both trait definition and impl need the macro" --tags rust,async
```

**Note:** Use double quotes to wrap multi-line text. On Windows CMD use `"`, on PowerShell/Bash escape inner quotes or use heredoc.

---

## Best Practices

### Content Quality

| Guideline | Description |
|-----------|-------------|
| Concise | 100-300 words per memory, max 500 words |
| Single topic | Each memory focuses on one core problem or topic |
| Structured | Use consistent template (context, solution, key points) |
| Specific tags | Precise classification for later filtering |
| Right granularity | Split if too large, merge if too fragmented |

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

### Common Mistakes

| ❌ Don't | ✅ Do |
|----------|--------|
| Record entire code files | Extract key snippets only |
| Record every answer | Only valuable insights |
| Record without checking | Search first to avoid duplicates |
| Skip when finding similar memory | Prioritize merging or updating existing memories |
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
memo update abc123 --content "Updated content" --tags rust,async

# Delete memory
memo delete abc123 --force

# Merge multiple memories
memo merge id1 id2 id3 --content "Merged summary content" --tags rust,cli
```

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

# Time filtering
memo search "What bugs were fixed recently" --after 2026-01-15

# Combined
memo search "What are common patterns for Vue components" -n 10 --after 2026-01-01 --threshold 0.6
```

### Split Memory Example

```bash
# Scenario: One memory about "CLI output format" with multiple aspects, already 600 words
# Original memory mixed: line control, error handling, output method rules

# Split into 3 independent memories:
memo embed "CLI output format - Line and separator control..." --tags rust,cli,output
memo embed "CLI error exit - exit vs bail choice..." --tags rust,cli,error
memo embed "CLI output rules - Prohibit direct println..." --tags rust,cli,output

# Delete original oversized memory
memo delete original-id --force
```

### Time-Based Search

```bash
# Recent work
memo search "database optimization experience" --after 2026-01-15

# Specific period
memo search "project progress" --after 2026-01-01 --before 2026-01-31 -n 20
```

---

## Trigger Phrases

| Action | Trigger Phrases |
|--------|---------|
| **Record** | "remember this", "record this", "save this", "summarize and record", "note this down" |
| **Search** | "how did we solve", "search memory", "do you remember", "recently...", "any similar experience", "have we done this before" |
