---
name: memo-brain
description: Manages and retrieves contextual memories across conversations using a vector database. Records experiences, solutions, and user habits. Provides context-aware assistance through semantic search. Use when needing to remember information, search memories, or when the user explicitly asks to "remember this" or "search memory".
---

# Memo Brain Management

Record and retrieve valuable knowledge using vector database semantic search.

**⚠️ Important: All commands require network access (for embedding API). When using Shell tool, set `required_permissions: ["network"]` or `["all"]`.**

## Quick Reference

| Command | Purpose | Example |
|---------|---------|---------|
| `memo embed <text>` | Record memory | `memo embed "Context:... Action:..." --tags rust,cli` |
| `memo search <query>` | Search memories | `memo search "how to fix MySQL timeout" -n 10` |
| `memo update <id>` | Update memory | `memo update abc123 --content "..." --tags rust` |
| `memo merge <ids>...` | Merge memories | `memo merge id1 id2 --content "..." --tags rust` |
| `memo delete <id>` | Delete memory | `memo delete abc123 --force` |
| `memo list` | List memories | `memo list` |

**Duplicate Detection:** When embedding detects similar content (similarity > 0.85), prioritize merging or updating.

---

## Five-Dimensional Memory Model

All memories are described using five dimensions (dimensions are optional, keep at least "Action" and "Result" or "Insight"):

```
Context → Background, situation, environment
Intent  → What to do, what to solve
Action  → What was done, approach, process
Result  → Outcome, effectiveness
Insight → What was learned, gotchas, reusable experience
```

**Simple Knowledge Example (Action + Insight):**

```bash
memo embed "Rust async-trait usage

Action: Add #[async_trait] macro to both trait and impl
Insight: Has slight performance overhead (Box allocation), suitable for non-critical paths" --tags rust,async
```

**Complete Problem-Solving Example (All Five Dimensions):**

```bash
memo embed "MySQL Connection Timeout - AWS Security Group

Context: Works locally, times out after deploying to AWS
Intent: Find root cause and fix connection failure
Action:
- Check connection pool config (max_connections, timeout) → No effect
- Restart MySQL → No effect
- Check MySQL logs, no connection records (key clue)
- Check AWS security group, port 3306 not open
Result: Connection restored after opening port 3306 in security group
Insight: Cloud servers close all ports by default, must check security group before deployment" --tags mysql,cloud,debug
```

**Flexibility Principles:**
- Dimensions are optional, order is flexible, labels can be omitted
- Five dimensions are a thinking framework, not a format template
- Goal is clear content, not format compliance

See [examples.md](examples.md) for more examples

---

## Core Workflows

### When to Record

**Should record:**
- Solved complex problem (complete troubleshooting process)
- Made technical decision (solution comparison and rationale)
- Discovered valuable knowledge (reusable experience)
- User explicitly asks ("remember this", "record this")

**Recognition signals (trigger complete recording):**
- Debugging, troubleshooting, fixing process in conversation
- Tried multiple approaches ("tried X but didn't work")
- Discussed tech selection, architectural decisions
- User says "struggled for long time", "finally solved"

**Should not record:**
- Simple syntax queries, common knowledge
- Duplicate content (search first, prioritize merge/update)

### When to Search

**Trigger scenarios:**
- Similar problem ("how did we solve this before")
- User explicitly asks ("search memory", "do you remember")
- Check related experience before new task
- Find recent work (use `--after`)

**Search principles (based on Five-Dimensional Model):**

Vector search relies on semantic understanding; queries should include sufficient context:
- ✅ Include **Context** (scenario) and **Intent** (goal): Describe your situation and what you want to solve
- ✅ Use complete question sentences: Like asking an experienced colleague
- ✅ Search automatically decomposes the query into sub-questions via LLM, searches in parallel, and synthesizes a comprehensive answer
- ❌ Don't just list keywords (e.g., "rust async trait")

**Query construction:**

| Intent Type | Query Construction | Example |
|-------------|-------------------|---------|
| Scenario Replay | Context + Symptoms + Problem | `memo search "MySQL connection keeps timing out after deploying to Alibaba Cloud, how to troubleshoot" -n 10` |
| Decision Recall | Context + Requirements + Decision Point | `memo search "memo-brain needs local embedded vector database, why choose LanceDB" -n 10` |
| Knowledge Query | Use Case + Technical Point | `memo search "Rust project needs async methods in traits, how to implement" -n 10` |

**Comparison examples:**

```bash
# ❌ Queries lacking context
memo search "why choose LanceDB"
memo search "MySQL timeout"

# ✅ Queries with context and intent
memo search "memo-brain needs local embedded vector database, why choose LanceDB" -n 10
memo search "MySQL connection keeps timing out after deploying to Alibaba Cloud, how to troubleshoot" -n 10
```

### Handling Duplicate Memories

Decision priority when similar memories detected (similarity > 0.85):

1. **Merge** - Content overlaps and can be consolidated (mind granularity)
2. **Update** - New content supplements existing memory (avoid oversizing)
3. **Split** - Existing memory is too large or has multiple topics
4. **Add New** - Confirmed as completely independent knowledge

```bash
# Update existing memory
memo update abc123 --content "..." --tags rust,async

# Merge similar memories
memo merge id1 id2 --content "..." --tags rust,error-handling

# Delete and re-embed
memo delete abc123 --force
memo embed "..." --tags rust,optimization
```

**Granularity control:**
- Each memory 100-400 words, max 600 words
- Single topic, focused on one core problem or experience
- Clear five-dimensional structure, easy to understand

---

## Tagging Strategy

Use multi-dimensional tags for classification and filtering (3-6 tags optimal):

| Dimension | Example |
|-----------|---------|
| Tech Stack | `rust,async,tokio` |
| Scenario | `debug,performance,security` |
| Importance | `important,decision,pitfall` |
| Project | `memo-brain,project-x` |

**Principles:**
- Include tech point + scenario/type
- Avoid overly generic (e.g., "code", "fix")
- Use specific, distinctive tags (e.g., "mysql" not "database")

---

## Time Filtering

| Scenario | Command Example |
|----------|-----------------|
| Recent memories | `memo search "database optimization" --after 2026-01-20 -n 10` |
| Time range | `memo search "project progress" --after 2026-01-01 --before 2026-01-31 -n 10` |
| With time filtering | `memo search "recent bugs" --after 2026-01-25 -n 15` |

---

## Common Mistakes

| ❌ Don't | ✅ Do |
|----------|--------|
| Record entire code files | Extract key snippets and approach |
| Record every answer | Only valuable insights and complete process |
| Record without checking | Search first to avoid duplicates |
| Skip when finding similar | Prioritize merge or update |
| Use vague titles | Be specific and descriptive |
| Too many generic tags | Keep tags focused and distinctive |
| Keyword-only search | Use complete question sentences |
| Limiting results too much | Increase `-n` parameter to get more related memories |
| Force five-dimensional format | Natural expression, dimensions optional |

---

## Trigger Phrases

| Action | Trigger Phrases |
|--------|-----------------|
| **Record** | "remember this", "record this", "summarize experience", "save this" |
| **Search** | "how did we do it", "search memory", "do you remember", "recently...", "any similar experience" |
