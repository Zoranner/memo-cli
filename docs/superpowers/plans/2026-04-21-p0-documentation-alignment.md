# Archived P0 Documentation Alignment Plan

This plan file is kept only for audit history.

It should not be used to drive current implementation work.

## Superseded By

- [`docs/architecture/command-philosophy.md`](../../architecture/command-philosophy.md)
- [`docs/architecture/memory-engine-architecture.md`](../../architecture/memory-engine-architecture.md)

## Why it was superseded

The original plan targeted a repository state that still referenced root-level `NEXT.md` and `ROADMAP.md`. Those files are no longer the planning surface for `memo-brain`.

Current implementation work should:

- compare behavior directly against the architecture documents above
- verify with current code under `src/` and `crates/engine/`
- avoid reviving the removed `ROADMAP.md` / `NEXT.md` workflow

## Historical Note

The original plan was useful while the repository was transitioning away from older entry-point docs, but it now risks teaching the wrong mental model to future agent runs. This archived version exists solely to make that boundary explicit.
