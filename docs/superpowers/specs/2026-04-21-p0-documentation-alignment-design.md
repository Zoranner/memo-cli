# Archived P0 Documentation Alignment Design

This file is retained only as a historical record of one documentation cleanup task.

It is no longer a source of truth for `memo-brain`.

## Current Truth Sources

- [`docs/architecture/command-philosophy.md`](../../architecture/command-philosophy.md)
- [`docs/architecture/memory-engine-architecture.md`](../../architecture/memory-engine-architecture.md)
- current code in `src/main.rs` and `crates/engine`

## Why this file is archived

The original version of this document assumed root-level `NEXT.md` and `ROADMAP.md` were still the active planning inputs. That is no longer true.

The architecture model has since been consolidated into the two documents above, and new work should be evaluated directly against those documents plus current code behavior.

## Historical Scope

The archived task covered:

- `README.md`
- `docs/zh-CN/README.md`
- an older `ROADMAP.md` workflow that has since been removed

If any guidance in older session notes or agent plans conflicts with the current architecture docs, treat the architecture docs as authoritative.
