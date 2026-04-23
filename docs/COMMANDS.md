# Command Reference

Current command reference for the local single-process memo engine.

[中文](zh-CN/COMMANDS.md) | English

## Public Commands

[Command Philosophy](architecture/command-philosophy.md) defines the public command language standard. This document follows that public surface directly.

- `memo awaken`
- `memo remember`
- `memo recall`
- `memo reflect`
- `memo dream`
- `memo state`
- `memo restore`

---

## `memo awaken`

Initialize the data directory and write template config files.

### Syntax

```bash
memo awaken [path]
```

### Output

Prints a human-readable summary with:

- awaken target directory
- whether `config.toml` was created or kept
- whether `providers.toml` was created or kept

### Notes

- `memo awaken [path]` also marks that directory as the active workspace memory store for later commands run from that directory or any descendant directory
- set `MEMO_DATA_DIR` to override the active workspace target explicitly for the current process

---

## `memo remember`

Write one episode into SQLite, update L0/session state, and mark derived indexes as pending.

### Syntax

```bash
memo remember <content> [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `--time <rfc3339>` | Observation timestamp |
| `--entity <type:name[:alias1|alias2]>` | Add manual entities |
| `--fact <subject:predicate:object>` | Add manual facts |
| `--dry-run` | Preview the final remember payload without writing |
| `--json` | Emit machine-readable output |

### Notes

- `--dry-run` prints the final remember payload before writing
- default `memo remember` writes only manual entities and facts immediately
- `--dry-run` may include provider-backed extraction when the user has explicitly configured an extraction provider
- by default commands use the nearest active workspace memory store chosen by the latest `memo awaken` in the current directory or one of its ancestors; `MEMO_DATA_DIR` overrides it

---

## `memo recall`

Query the engine. By default it runs the fast path, and it may auto-escalate to deep search when results look ambiguous.

### Syntax

```bash
memo recall <query> [OPTIONS]
```

### Options

| Option | Description |
| --- | --- |
| `-n, --limit <n>` | Result limit, default `10` |
| `--deep` | Force deep search immediately |
| `--json` | Emit machine-readable output |

### Notes

- Deep search can trigger rerank when configured
- Output includes `deep_search_used` and per-result `reasons`

---

## `memo reflect`

Inspect one memory record by id.

### Syntax

```bash
memo reflect <id> [--json]
```

---

## `memo dream`

Dream entrypoint.

### Syntax

```bash
memo dream [--full] [--json]
```

### Behavior

- Default mode runs one manual dream pass
- `--full` runs a fuller dream pass with an extra stabilization pass when the first pass changes memory state
- when an extraction provider is configured, dream can enrich still-unstructured episodes on the slow path without changing `remember` default latency
- `--json` emits machine-readable output

---

## `memo state`

### Syntax

```bash
memo state [--json]
```

### Includes

- episode / entity / fact / edge counts
- layer and cache status
- derived index health
- provider runtime health, including the latest degraded capability summary when fallback paths were used
- maintenance status

---

## `memo restore`

Recover derived layers from the local truth source when needed.

### Syntax

```bash
memo restore [--full] [--json]
```

### Notes

- Default mode performs a conservative restore
- `--full` rebuilds derived layers completely from the truth source
- `--json` emits machine-readable output


