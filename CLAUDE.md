# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

- Build workspace: `cargo build --all-features`
- Run all tests: `cargo test --all-features`
- Run a specific integration test file: `cargo test -p memo-engine --test engine_flow`
- Run a specific engine test: `cargo test -p memo-engine --test engine_flow consolidation_promotes_repeated_fact_support_to_l3_without_query_heat`
- Run a specific CLI unit test: `cargo test cli_parses_extract_command`
- Run formatting: `cargo fmt --all`
- Run lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Show CLI help: `cargo run -- --help`
- Show subcommand help: `cargo run -- query --help`
- Initialize local data/config in default data dir: `cargo run -- init`
- Query against a local data dir: `cargo run -- --data-dir .memo query "your query" --deep`
- Refresh derived indexes after ingest: `cargo run -- --data-dir .memo refresh-index --scope all`
- Run consolidation immediately: `cargo run -- --data-dir .memo dream --trigger manual`
- Enqueue consolidation job: `cargo run -- --data-dir .memo dream --trigger session_end --enqueue`
- Consume queued consolidation jobs: `cargo run -- --data-dir .memo run-dream-jobs --limit 10`

## Required workflow

For any Rust code changes, always run in this order before finishing:

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo build --all-features` if code paths changed materially or new APIs were added
4. `cargo test --all-features` or the narrowest relevant test target while iterating, then full tests before claiming completion

## Architecture

This repository is a Rust workspace with three important layers:

- Root crate `memo`: CLI entrypoint and local app wiring. `src/main.rs` defines commands and translates CLI input into `memo-engine` calls. `src/app_config.rs` resolves `.memo/config.toml` and `.memo/providers.toml`, builds provider adapters, and initializes local data directories.
- `crates/engine`: stateful local memory engine. It owns SQLite persistence, Tantivy text search, HNSW vector search, retrieval ranking, layer promotion/cooling, and consolidation job orchestration.
- `crates/lmkit`: reusable multi-provider AI client library. The root crate wraps it via adapters instead of letting `memo-engine` depend on network providers directly.

Keep that boundary intact: provider/network concerns belong in root crate + `lmkit`, while storage/retrieval/consolidation logic belongs in `memo-engine`.

## Core data model

`memo-engine` stores four record kinds in SQLite truth storage (`crates/engine/src/db.rs`):

- `Episode`: raw remembered text
- `Entity`: canonicalized named thing with aliases
- `Fact`: structured triple-like statement
- `Edge`: graph relation derived from facts

Each record also carries a memory layer in `crates/engine/src/types.rs`:

- `L1`: fresh observations
- `L2`: reinforced/promoted memory
- `L3`: stable long-term memory used for hot cache behavior and stronger retrieval boosts

Layer transitions are not cosmetic. Query ranking and consolidation both depend on them.

## Ingest pipeline

`memo ingest` and engine ingestion follow this flow:

1. CLI parses raw content plus optional manual `--entity` / `--fact` inputs in `src/main.rs`.
2. `MemoryEngine::preview_ingest` merges manual structured input with optional extraction-provider output.
3. `MemoryEngine::ingest_episode` writes episode/entity/fact/edge records into SQLite.
4. Embeddings are generated opportunistically when an embedding provider is configured.
5. Derived indexes are marked pending, not necessarily rebuilt immediately.
6. Session cache and L3 cache are refreshed.

Important: current architecture treats SQLite as source of truth and text/vector indexes as derived state. When debugging mismatches, inspect DB truth first, then refresh or rebuild indexes.

## Retrieval pipeline

`MemoryEngine::query` in `crates/engine/src/engine.rs` is multi-stage and should be understood before changing ranking behavior.

Fast path candidates can come from:

- L0/session cache matches
- L3 hot cache matches
- exact alias/exact text matches from SQLite
- BM25 hits from Tantivy text index
- vector similarity hits from HNSW index when embeddings are enabled
- graph expansion from related entity/fact/edge records

Then engine applies score shaping:

- recency boost
- layer boost
- hit-frequency boost
- optional rerank provider, but only in deep search
- final MMR selection to reduce near-duplicate top results

Non-deep queries may auto-escalate to deep search when results look ambiguous. If ranking changes seem surprising, inspect whether auto-escalation or rerank was triggered before changing weights.

## Consolidation and maintenance

Consolidation logic lives in `MemoryEngine::consolidate` / `run_consolidation`.

Key behaviors:

- deduplicate L1 episode clusters
- promote supported episodes/entities/facts/edges from L1 to L2
- promote sufficiently reinforced memory to L3
- invalidate conflicting facts and matching edges
- cool stale L3 records back to L2
- refresh L3 cache after consolidation changes
- support queued consolidation jobs in SQLite, not only synchronous runs

If you touch promotion/cooling rules, read the engine integration tests first. Much of project intent is encoded there.

## Configuration and provider wiring

Default CLI data dir is `.memo` (`src/main.rs`). `memo init` writes local `config.toml` and `providers.toml` templates into that data dir.

Provider references use `<provider>.<service>` syntax such as `openai.embed` or `aliyun.rerank`. Resolution happens in `src/app_config.rs` by reading the local data-dir config files, not global SDK env conventions.

Current app config parser is hand-rolled and intentionally narrow. If adding config keys, update parsing logic and tests in `src/app_config.rs` instead of assuming full TOML semantics already exist.

## Testing focus

Most behavioral coverage lives in:

- `crates/engine/tests/engine_flow.rs` for end-to-end engine behavior, ranking, consolidation, index maintenance, and queued jobs
- `src/app_config.rs` tests for local config/provider resolution
- `src/main.rs` tests for CLI parsing and rendering helpers
- `src/lmkit_extraction_adapter.rs` tests for extraction JSON normalization/cleanup

Prefer extending those existing suites over creating parallel test harnesses.

## Documentation gotcha

`README.md` still contains older `memo search` / `memo embed` examples and older capability wording. For current CLI behavior and command surface, trust these first:

- `src/main.rs`
- `docs/COMMANDS.md`
- engine integration tests in `crates/engine/tests/engine_flow.rs`

If future work changes CLI semantics, update `README.md` and `docs/COMMANDS.md` together so they do not drift further.
