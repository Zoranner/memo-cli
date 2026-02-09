# Memory Examples

Detailed usage examples of the Five-Dimensional Memory Model.

---

## Example Categories

- [Simple Knowledge](#simple-knowledge-minimal) - Just "Action" and "Insight"
- [Problem Solving](#problem-solving-all-five-dimensions) - Complete troubleshooting process
- [Technical Decision](#technical-decision-focus-on-intent-action-insight) - Solution comparison and choice
- [Configuration](#configuration-focus-on-action) - Configuration and commands
- [Progressive Recording](#progressive-recording-from-simple-to-rich) - Gradually refining memories

---

## Simple Knowledge (Minimal)

Quick recording of API usage, configuration, etc. Usually just "Action" and "Insight".

### Example 1: Rust async-trait

```bash
memo embed "Rust async-trait usage

Action: Add #[async_trait] macro to both trait definition and impl blocks
Insight: Has slight performance overhead (Box allocation), suitable for non-critical paths" --tags rust,async
```

### Example 2: Natural Expression (Without Dimension Labels)

```bash
memo embed "Rust async-trait usage

Add #[async_trait] macro to both trait and impl to enable async fn in traits.

Note: Has slight performance overhead (Box allocation), suitable for non-critical paths." --tags rust,async
```

---

## Problem Solving (All Five Dimensions)

Record complete troubleshooting and resolution process using all five dimensions.

### Example: MySQL Connection Timeout Troubleshooting

```bash
memo embed "MySQL Connection Timeout - AWS Security Group Issue

Context: Local development works fine, database connection times out after deploying to AWS EC2

Intent: Find root cause of connection failure and resolve it

Action: Systematically troubleshooted following standard approach
1. Check connection pool config (max_connections, timeout) → Normal parameters, no effect
2. Restart MySQL service → No effect
3. Check MySQL error logs → No connection records at all (key clue)
4. Realized it might be network layer issue, checked AWS security group configuration
5. Found inbound rule missing for port 3306

Result: Connection immediately restored after adding inbound rule to open port 3306 in security group

Insight: Cloud servers close all ports by default, must check before deploying:
- Database ports (MySQL 3306, PostgreSQL 5432)
- Application ports (Web services 80/443/8080, etc.)
- If MySQL logs show no connection records, check network layer first (firewall/security group)" --tags mysql,cloud,debug,network
```

---

## Technical Decision (Focus on Intent, Action, Insight)

Record technical selection and architectural decisions, focusing on comparison analysis and conclusions.

### Example: Vector Database Selection

```bash
memo embed "Vector Database Selection - Why LanceDB

Context: memo-brain needs local embedded vector database, requires high performance and easy deployment

Intent: Choose the most suitable solution from multiple vector database options

Action: Evaluated three mainstream options
- ChromaDB: Lightweight, simple Python/JS API, but mediocre performance, pure Python implementation
- LanceDB: Rust native, high performance, ACID transaction support, multimodal, but slightly steeper learning curve
- Qdrant: Feature-rich, enterprise-grade, but requires standalone service deployment, not suitable for embedded scenarios

Result: Chose LanceDB

Insight: Key considerations for embedded scenarios
- Prioritize performance (vector retrieval is core operation)
- Deployment convenience matters (single binary > standalone service)
- Language ecosystem match (Rust project benefits from Rust library, more natural integration)
- LanceDB's Lance columnar format and HNSW indexing ensure performance" --tags architecture,decision,vector-db
```

---

## Configuration (Focus on Action)

Record configuration, commands, and operational knowledge.

### Example: Cargo Workspace Dependency Management

```bash
memo embed "Cargo workspace dependency management

Action: Configure in root Cargo.toml

[workspace]
members = [\"types\", \"local\", \"cli\"]

[workspace.dependencies]
serde = { version = \"1\", features = [\"derive\"] }
tokio = { version = \"1\", features = [\"rt-multi-thread\"] }

Reference in child crates:
[dependencies]
serde.workspace = true

Insight: Workspace unifies dependency versions, avoids version conflicts, reduces compile time" --tags rust,cargo,config
```

---

## Progressive Recording (From Simple to Rich)

Demonstrates how to start with quick recording and gradually refine memories.

### Step 1: Quick Record Core Content

```bash
memo embed "Memory tree search implementation

Action: Using recursive vector search + funnel threshold strategy
- Layer 1: 0.60 (cast wide net)
- Subsequent layers increment: 0.10/0.07/0.05
- Tag filtering + global deduplication

Result: Implemented automatic associative hierarchical search without manual depth specification" --tags memo-cli,algorithm,vector-search
```

### Step 2: Add Background and Insights

```bash
memo update abc123 --content "Memory tree search implementation

Context: Pure vector search only finds directly related memories, cannot discover indirectly associated knowledge networks

Intent: Implement automatic associative recursive search so AI can discover complete knowledge chains

Action: Using recursive vector search + funnel threshold strategy
- Layer 1: 0.60 (cast wide net, more relaxed than regular search)
- Subsequent layers increment: Adaptively adjust increment based on starting threshold
- Tag filtering: Starting from layer 2, require at least one overlapping tag
- Global deduplication: visited set ensures each memory appears only once

Result: Implemented automatic associative hierarchical search without manual depth specification

Insight:
- Funnel threshold is key: relaxed start ensures recall, gradual tightening ensures precision
- Vectors themselves contain semantic information, can be used directly as search seeds (no need to extract keywords)
- This is a unique feature that other vector databases (Mem0/ChromaDB) don't have" --tags memo-cli,algorithm,vector-search,innovation
```

---

## More Scenarios

### Performance Optimization Record

```bash
memo embed "Batch embedding performance optimization

Context: memo embed is slow when processing many files, needs to call API one by one
Intent: Improve batch embedding performance
Action:
- Use rayon to parallelize file scanning
- Batch call embedding API (reduce network round trips)
- Database batch insert (insert_batch)
Result: Expected 30-50% performance improvement
Insight: Three optimization directions for I/O intensive tasks: file scanning, API calls, database writes" --tags rust,performance,rayon
```

### Architecture Refactoring Record

```bash
memo embed "Configuration loading logic refactor

Context: Each service module repeatedly implements config loading, code duplication
Intent: Extract common config loading pattern
Action: Created ConfigLoader struct
- load() method: Load config by priority (CLI args > local > global > default)
- validate() method: Validate required fields
- All services reuse same logic
Result: Eliminated 200+ lines of duplicate code
Insight: Identifying and extracting repetitive patterns is the first step in refactoring, priority system is core of config management" --tags rust,refactoring,design-pattern
```

### Bug Fix Record

```bash
memo embed "Update operation data safety fix

Context: memo update deletes old memory then inserts new one, data loss risk if insert fails after delete
Intent: Ensure atomicity of update operation
Action: Changed to insert new memory first, then delete old one after success
- Insert fails: Original memory unaffected
- Delete fails: Log error but keep new memory
Result: Eliminated data loss risk
Insight: Operations involving data modification must consider failure scenarios, create-then-delete is safer than delete-then-create" --tags rust,bug-fix,data-safety
```

---

## Usage Tips

### Choose Dimensions Based on Content

Not all memories need all five dimensions:
- **Pure knowledge**: Action + Insight (e.g., API usage)
- **Quick Q&A**: Intent + Action (e.g., configuration questions)
- **Complete case**: Context + Intent + Action + Result + Insight (e.g., complex bug troubleshooting)

### Dimensions Can Be Merged

```bash
# Result and Insight can be merged
"Chose LanceDB for its performance and easy integration (Result+Insight)"

# Context and Intent can flow naturally
"Encountered MySQL timeout, needed quick diagnosis (Context+Intent)"
```

### Don't Force Dimension Labels

```bash
# With labels (clear structure)
"Context:... Action:... Insight:..."

# Without labels (natural flow)
"Encountered problem... Tried several approaches... Finally discovered..."
```

Both work - goal is content clarity.

### Titles Should Include Key Information

```bash
✅ Good title: "MySQL Connection Timeout - AWS Security Group Issue"
   Includes: problem + root cause

❌ Poor title: "Database Problem"
   Too vague, hard to identify when searching
```
