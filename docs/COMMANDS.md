# Command Reference

Current command reference for the local single-process memo engine.

[中文](zh-CN/COMMANDS.md) | English

## Available Commands

- `memo init` — initialize local config templates
- `memo extract` — run the configured extraction provider directly
- `memo ingest` — write an episode into SQLite truth source
- `memo query` — query the engine with fast-path or deep search
- `memo inspect` — inspect one memory record by id
- `memo dream` — run or enqueue consolidation jobs
- `memo run-dream-jobs` — consume queued consolidation jobs
- `memo refresh-index` — refresh pending derived indexes only
- `memo rebuild-index` — force full rebuild of derived indexes
- `memo stats` — inspect engine and queue status
- `memo benchmark` — run repeated query benchmarks

---

## `memo init`

Initialize the data directory and write template config files.

### Syntax

```bash
memo init [--data-dir <path>]
```

### Output

Prints JSON with:

- `data_dir`
- `config_created`
- `providers_created`

---

## `memo extract`

Call the configured extraction provider directly without writing memory.

### Syntax

```bash
memo extract <content> [--data-dir <path>]
```

### Notes

- Requires `[extract] extraction_provider` in `config.toml`
- Returns pretty JSON for extracted entities and facts

---

## `memo ingest`

Write one episode into SQLite, update L0/session state, and mark derived indexes as pending.

### Syntax

```bash
memo ingest <content> [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `--layer <L1|L2|L3>` | Target memory layer, default `L1` |
| `--session <id>` | Optional session id |
| `--at <rfc3339>` | Optional observation timestamp |
| `--entity <type:name[:alias1|alias2]>` | Add manual entities |
| `--fact <subject:predicate:object>` | Add manual facts |
| `--dry-run` | Preview merged ingest payload without writing |

### Notes

- `--dry-run` prints the final ingest preview after manual input + provider extraction merge
- Actual index refresh is now explicit; ingest only marks `text` / `vector` indexes as `pending`

---

## `memo query`

Query the engine. By default it runs the fast path, and it may auto-escalate to deep search when results look ambiguous.

### Syntax

```bash
memo query <query> [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `-n, --limit <n>` | Result limit, default `10` |
| `--deep` | Force deep search immediately |

### Notes

- Deep search can trigger rerank when configured
- Output includes `deep_search_used` and per-result `reasons`

---

## `memo inspect`

Inspect one memory record by id.

### Syntax

```bash
memo inspect <id> [--data-dir <path>]
```

---

## `memo dream`

Consolidation entrypoint.

### Syntax

```bash
memo dream [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `--trigger <manual|idle|session_end|before_compaction>` | Consolidation trigger, default `manual` |
| `--enqueue` | Queue a pending consolidation job instead of running immediately |

### Behavior

- Default mode runs consolidation synchronously
- `--enqueue` returns a pending `job_id` without executing the job

---

## `memo run-dream-jobs`

Consume queued consolidation jobs.

### Syntax

```bash
memo run-dream-jobs [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `--limit <n>` | Max queued jobs to run, default `1` |

---

## `memo refresh-index`

Refresh only indexes currently marked as pending.

### Syntax

```bash
memo refresh-index [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `--scope <all|text|vector>` | Refresh scope, default `all` |

### Notes

- This is the normal post-ingest maintenance entrypoint
- If the selected index is not pending, the command returns an empty rebuild report

---

## `memo rebuild-index`

Force a full rebuild of derived indexes from SQLite truth source.

### Syntax

```bash
memo rebuild-index [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `--scope <all|text|vector>` | Rebuild scope, default `all` |

### Notes

- Use this for recovery or explicit full rebuilds
- Unlike `refresh-index`, this does not require pending state

---

## `memo stats`

Print engine status.

### Syntax

```bash
memo stats [--data-dir <path>]
```

### Includes

- episode / entity / fact / edge counts
- L3 cache size
- text / vector index status
- consolidation job queue counts (`pending`, `running`, `completed`, `failed`)

---

## `memo benchmark`

Run repeated query benchmarks.

### Syntax

```bash
memo benchmark <query> [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--data-dir <path>` | Use a specific data directory |
| `--iterations <n>` | Number of iterations, default `20` |
| `-n, --limit <n>` | Query limit, default `10` |

### Notes

- Benchmark currently runs default non-forced-deep queries
- Output includes average and total elapsed milliseconds
