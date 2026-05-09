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

---

## `memo awaken`

Initialize the data directory and write template config files.

### Syntax

```bash
memo awaken
```

### Output

Prints a human-readable summary with:

- awaken target directory
- fixed config directory
- whether `config.toml` was created or kept
- whether `providers.toml` was created or kept

### Notes

- `memo awaken` always keeps `config.toml` and `providers.toml` under `~/.memo`
- by default the data directory is `~/.memo/data`
- set `MEMO_DATA_DIR` or `storage.data_dir` in `~/.memo/config.toml` to override the data directory

---

## `memo remember`

Write one episode into SQLite and mark derived indexes as needing `memo dream` maintenance when needed.

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
| `--json` | Emit machine-readable output |

### Notes

- default `memo remember` writes only manual entities and facts immediately
- `--entity` and `--fact` are advanced structured inputs; normal users can write natural-language episodes and let `memo dream` structure them later when extraction is configured
- by default commands use `~/.memo/data`; `MEMO_DATA_DIR` overrides `storage.data_dir`, and `storage.data_dir` overrides the default

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

- Default recall reads local memory state and should not require provider calls
- Output includes `deep_search_used` and per-result `reasons`
- Recall diagnostics use precise local-search semantics: `provider_calls=0` means no provider was called by this command; `total_candidates` is the unique pre-selection candidate pool, not raw hits; `capabilities` describes candidate pool sources, not necessarily final result `reasons`
- `working_set` is a local context candidate and weighting source; a Working Set hit does not mean the text or vector index is ready

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
- if extraction is missing, degraded, or still using a template placeholder key, dream reports that unstructured episodes remain text-only instead of pretending semantic structuring is available
- dream is also the public maintenance entrypoint for derived text/vector layers; internal repair details stay diagnostics, not a separate user workflow
- `--json` emits machine-readable output

---

## `memo state`

### Syntax

```bash
memo state [--json]
```

### Text Output

Text output exposes only the user-facing action contract:

- `status`: one of `ready`, `needs_setup`, `needs_dream`
- `message`: short human-readable reason
- `next`: one of `none`, `configure provider`, `memo dream`

Provider setup problems, including missing providers and placeholder keys, become `status: needs_setup` with `next: configure provider`.

Unstructured content, missing local semantic material, or unsynced/untrusted derived layers become `status: needs_dream` with `next: memo dream`.

### JSON Output

`--json` keeps the same top-level `status`, `message`, and `next`, and adds `diagnostics` for internal details:

- `diagnostics.internal_reasons`, such as `provider_not_ready`, `needs_structure`, `needs_vectors`, `sync_needed`, `full_refresh_needed`
- engine state counts and index status
- provider readiness and runtime health

Internal index bookkeeping such as index jobs/index state is diagnostics only and is not part of the text status line.


