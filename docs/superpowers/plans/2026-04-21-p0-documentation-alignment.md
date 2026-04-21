# P0 Documentation Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align repository entry-point documentation with the current local single-process memory engine and remove old `memo search` / `memo embed` product framing.

**Architecture:** This plan updates only documentation files and uses current CLI code plus command reference docs as the source of truth. The work is split by document responsibility: English README, Chinese README, then roadmap state reconciliation.

**Tech Stack:** Markdown, Rust CLI source (`src/main.rs`), existing command docs, git

---

## File map

- Modify: `README.md` — rewrite capability framing and quick-start usage to current CLI/engine model
- Modify: `docs/zh-CN/README.md` — mirror README changes in Chinese without adding divergent claims
- Modify: `ROADMAP.md` — refresh stale current-state sections without changing long-term roadmap phases
- Reference: `src/main.rs` — canonical current CLI command list
- Reference: `docs/COMMANDS.md`
- Reference: `docs/zh-CN/COMMANDS.md`
- Reference: `NEXT.md`

### Task 1: Refresh English README

**Files:**
- Modify: `README.md`
- Reference: `src/main.rs:27-83`
- Reference: `docs/COMMANDS.md:6-17`

- [ ] **Step 1: Replace outdated capability framing**

Rewrite the capability table so it describes the current engine rather than the old search/embed product UX.

Required concepts to include:

```md
| Capability | Description |
|------------|-------------|
| Local truth source | SQLite stores episodes, entities, facts, edges, and job/index state |
| Hybrid retrieval | Query combines exact/alias/BM25/vector/graph signals with optional deep search |
| Structured remember | `memo remember` can merge manual facts/entities with provider extraction |
| Dream workflows | `memo dream` promotes, cools, and reconciles memory layers |
| Rebuildable indexes | Text/vector indexes are derived and can be refreshed or rebuilt from SQLite |
| Provider-backed AI hooks | Extraction, embedding, and rerank are wired through provider config |
```

- [ ] **Step 2: Replace outdated quick-start example**

Remove old examples that use `memo search` and `memo embed`. Replace them with a short current workflow such as:

```md
```bash
memo awaken
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
memo recall "Where does Alice live?"
memo dream --trigger manual
memo restore
```
```

And explain the flow in prose:

```md
This workflow initializes a local data directory, remembers one episode into SQLite, queries the engine through the current retrieval pipeline, runs dream, and refreshes any pending derived indexes.
```

- [ ] **Step 3: Tighten configuration wording**

Keep installation and config sections, but ensure wording matches current architecture:

```md
- Local config lives under the selected data dir (default `.memo`)
- `memo awaken` writes `config.toml` and `providers.toml` templates into that data dir
- provider references use `<provider>.<service>` names such as `openai.embed` or `aliyun.rerank`
```

- [ ] **Step 4: Verify command names against source of truth**

Run:

```bash
grep -n "enum Command" -n src/main.rs
cargo run -- --help
```

Expected:
- README only names commands that exist in current CLI
- no README primary example contains `memo search` or `memo embed`

### Task 2: Refresh Chinese README

**Files:**
- Modify: `docs/zh-CN/README.md`
- Reference: `README.md`
- Reference: `docs/zh-CN/COMMANDS.md:6-18`

- [ ] **Step 1: Mirror the English README structure**

Update the Chinese README to match the same sections and claims as `README.md`, keeping terminology stable.

Required terminology:

```md
- SQLite 真相源
- 派生索引
- fast path / deep search
- dream
- provider 配置
```

- [ ] **Step 2: Replace old CLI examples**

Remove old `memo search` / `memo embed` examples and replace them with a Chinese explanation of the current CLI flow.

Use commands like:

```bash
memo awaken
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
memo recall "Alice 住在哪里？"
memo dream --trigger manual
memo restore
```

- [ ] **Step 3: Keep claims aligned with English README**

Ensure the Chinese file does not introduce extra promises such as features that only exist in the old product framing.

Checklist:

```md
- no primary example with `memo search`
- no primary example with `memo embed`
- command list matches current CLI
- wording describes current engine, not a separate Chinese-only product model
```

- [ ] **Step 4: Verify parity with English README**

Run a manual diff review:

```bash
git diff -- README.md docs/zh-CN/README.md
```

Expected:
- structure and claims align semantically
- language differs, product meaning does not

### Task 3: Refresh roadmap current-state assessment

**Files:**
- Modify: `ROADMAP.md`
- Reference: `NEXT.md`
- Reference: `crates/engine/src/vector_index.rs`
- Reference: recent commits `d4cb8e9`, `6409baf`

- [ ] **Step 1: Update stale current snapshot bullets**

Edit the “当前实现仍有这些明显限制” and adjacent current-state sections to reflect current code.

Required direction:

```md
- 向量检索不再是单纯 JSON + 内存全量余弦扫描，现已具备 HNSW 持久化索引，但距离 NEXT 的最终本地 ANN 目标仍未完全收口
- rerank 已接到配置装配与 deep retrieval 流程，但仍不是最终本地优先形态
- deep search 已有自动升级基础，但还不是完整策略驱动系统
- 文档收口仍未完成，README 与当前实现仍需继续对齐
```

- [ ] **Step 2: Update status table conservatively**

Adjust only stale rows in the “状态总览” table.

Recommended final states:

```md
| 向量 ANN 索引 | 部分完成 | 已接入持久化 HNSW 索引，但仍未达到 NEXT 目标中的最终本地 ANN 路线 |
| 本地 embedding / rerank / extraction | 未完成 | rerank 已接通，但整体仍主要依赖远端 provider |
| Deep Search 按需升级 | 部分完成 | 已有自动升级基础，但还不是完整策略驱动 |
| 文档与实现对齐 | 部分完成 | COMMANDS 与 CLAUDE 已较新，README 仍需继续收口 |
```

- [ ] **Step 3: Preserve roadmap scope**

Do not rewrite later phases. Keep distinctions between current partial progress and final target state from `NEXT.md`.

Quick review prompt:

```md
- Did any Phase section change because of impatience rather than stale status? If yes, revert it.
- Did any “部分完成” accidentally become “已完成” without matching NEXT goals? If yes, downgrade it.
```

- [ ] **Step 4: Verify recent feature alignment**

Run:

```bash
git log --format="%h %s" -n 5
```

Expected:
- roadmap current-state text no longer contradicts rerank wiring and persisted HNSW support

### Task 4: Validate documentation-only scope

**Files:**
- Modify: `README.md`
- Modify: `docs/zh-CN/README.md`
- Modify: `ROADMAP.md`

- [ ] **Step 1: Review final diff**

Run:

```bash
git diff -- README.md docs/zh-CN/README.md ROADMAP.md
```

Expected:
- only the three planned documentation files changed for P0 implementation
- README files contain no primary `memo search` / `memo embed` usage examples
- roadmap status is updated but phase structure remains intact

- [ ] **Step 2: Run formatting-independent sanity checks**

Run:

```bash
grep -R "memo search\|memo embed" README.md docs/zh-CN/README.md || true
```

Expected:
- no matches in the two README files, or only clearly marked historical references if intentionally retained

- [ ] **Step 3: Commit documentation changes**

Stage and commit only the three documentation files.

```bash
git add README.md docs/zh-CN/README.md ROADMAP.md
git commit -m "Align README and roadmap with current engine"
```

Expected:
- one documentation-only commit with a subject matching repository style

## Self-review

- Spec coverage: README, Chinese README, and roadmap current-state reconciliation are each covered by a dedicated task.
- Placeholder scan: No TBD/TODO placeholders remain.
- Consistency: All tasks use the same command set and same scope boundary.









