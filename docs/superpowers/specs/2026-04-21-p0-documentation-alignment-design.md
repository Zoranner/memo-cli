# P0 Documentation Alignment Design

**Date:** 2026-04-21  
**Scope:** `README.md`, `docs/zh-CN/README.md`, `ROADMAP.md`

## Goal

Complete P0 documentation alignment so that repository entry-point docs reflect the current local single-process memory engine instead of the older `memo search/embed` product model.

## Why this work is needed

The repository now exposes the current CLI through `src/main.rs` and documents it accurately in `docs/COMMANDS.md`, but the English and Chinese README files still present the older command surface and capability framing. `ROADMAP.md` also contains status statements that were true before recent retrieval/indexing changes but are no longer fully accurate.

If left unchanged, future contributors and Claude sessions will form the wrong mental model:

- they may assume the primary workflow is `memo search` / `memo embed`
- they may miss the current ingest/query/consolidation/index maintenance flow
- they may misread roadmap status for ANN, rerank, and deep search behavior

## Non-goals

This P0 does **not**:

- change implementation code
- rewrite `docs/COMMANDS.md` or `docs/zh-CN/COMMANDS.md` unless a blocking inconsistency is discovered
- redesign project positioning beyond what is needed to align docs with the current engine
- claim final-state `NEXT.md` goals are already complete when they are only partially implemented

## Source of truth

For this P0, documentation must align to these sources in priority order:

1. `src/main.rs` — current CLI command surface
2. `docs/COMMANDS.md` and `docs/zh-CN/COMMANDS.md` — current command semantics
3. recent commits on `master` — current feature status, especially:
   - `d4cb8e9 Add rerank retrieval and queued maintenance workflows`
   - `6409baf Persist HNSW vector index sidecar files`
4. current engine behavior described in:
   - `crates/engine/src/engine.rs`
   - `crates/engine/src/vector_index.rs`
   - `ROADMAP.md`
   - `NEXT.md`

If older README wording conflicts with the above, README should be updated rather than treated as authoritative.

## File-by-file design

### `README.md`

#### Keep

- installation section
- configuration section structure
- links to command reference, skill docs, config examples

#### Change

- replace the old product-style capability table with a capability summary that matches the current engine
- replace old `memo search` / `memo embed` usage examples with current CLI examples
- describe the repository as a local single-process memory engine with SQLite truth source and derived text/vector indexes
- mention current maintenance workflow explicitly: ingest marks indexes pending; refresh/rebuild and dream job processing are separate actions

#### Required content direction

The README should communicate these concepts clearly:

- SQLite is the truth source
- text and vector indexes are rebuildable derived layers
- current user workflow centers on `init`, `ingest`, `query`, `inspect`, `dream`, `refresh-index`, `rebuild-index`, `stats`, and `benchmark`
- extraction, embedding, and rerank are provider-backed features in the current architecture
- consolidation is now part of the product surface, not hidden behavior

#### Content to remove or rewrite

- examples that show `memo search`
- examples that show `memo embed`
- claims that imply the old multi-query memory product UX is still the primary CLI
- configuration descriptions for sections that no longer exist in current templates

### `docs/zh-CN/README.md`

#### Keep

- same high-level structure as English README
- installation/configuration/help links

#### Change

- mirror the English README’s updated structure and claims
- remove old `memo search` / `memo embed` examples
- use stable terminology consistent with the current implementation:
  - SQLite 真相源
  - 派生索引
  - consolidation
  - fast path / deep search
- avoid introducing extra claims that are not present in the English README

#### Translation rule

This file should be a faithful Chinese counterpart, not a divergent product description.

### `ROADMAP.md`

#### Keep

- overall roadmap purpose
- phase structure
- long-term direction from `NEXT.md`

#### Change

Update only the **current-state assessment** portions that are now stale.

#### Required status corrections

- vector index status should no longer describe the implementation as only JSON persistence plus in-memory full cosine scan; it must reflect persisted HNSW sidecar support
- rerank status should reflect that rerank is wired into config/retrieval flow, while still noting any remaining gaps relative to the final target architecture
- deep search status should reflect that there is already some automatic escalation behavior, even if the final policy-driven system is not complete
- documentation alignment should remain unfinished, but progress should reflect that command docs and CLAUDE guidance have been updated more recently than README

#### Constraint

Do **not** rewrite future phases merely because current status changed. The roadmap should still preserve the distinction between “partially implemented now” and “final target state from NEXT”.

## Validation criteria

P0 is complete when all of the following are true:

1. `README.md` contains no primary usage examples based on `memo search` or `memo embed`
2. `docs/zh-CN/README.md` contains no primary usage examples based on `memo search` or `memo embed`
3. every CLI command named in either README exists in `src/main.rs` and `docs/COMMANDS.md`
4. `ROADMAP.md` no longer contradicts recent feature work on:
   - persisted HNSW vector index support
   - rerank wiring
   - partial automatic deep-search escalation
5. changes are limited to documentation files in this P0

## Risks and mitigations

### Risk: overcorrecting toward final-state architecture

The docs could start describing the intended `NEXT.md` end state as if it already exists.

**Mitigation:** distinguish current implementation from target architecture explicitly. Use “current”, “today”, or “final target” wording where needed.

### Risk: English/Chinese README drift

One README may be updated more aggressively than the other.

**Mitigation:** update them in the same session using the same structure and command set.

### Risk: roadmap becomes too optimistic

Recent progress may tempt wording like “ANN complete” or “deep search complete”.

**Mitigation:** prefer “partially complete” whenever the implementation has moved forward but still falls short of `NEXT.md` target semantics.

## Out of scope follow-ups

These may follow after P0 but are not part of this design:

- improving CLI help strings for consistency with the refreshed docs
- restructuring `docs/COMMANDS.md`
- adding architecture diagrams
- refactoring `engine.rs`
- implementing local ONNX model execution

## Execution summary

Implement P0 as a documentation-only change set touching exactly:

- `README.md`
- `docs/zh-CN/README.md`
- `ROADMAP.md`

Use current code and command docs as truth. Remove old CLI examples, align repository positioning to the current engine, and update roadmap status text without expanding scope into implementation work.
