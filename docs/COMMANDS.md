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

Prints JSON with:

- `data_dir`
- `config_created`
- `providers_created`

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
- Structured entities and facts can come from manual flags and optional provider extraction

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

### Options

### Behavior

- Default mode runs ordinary consolidation
- `--full` represents a more complete dream pass
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


