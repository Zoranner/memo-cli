# Source Layout Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `memo-brain` 当前半完成的源码目录迁移收敛成一套清晰、可维护、可验证的模块结构。

**Architecture:** 根级 `src` 收敛为 CLI / config / providers 三层，并继续细分内部大文件。`crates/engine/src` 保持 engine-first 架构，但把 `db` 的 schema、index jobs、mappers、tests 等从巨型 `mod.rs` 中拆出。整个过程保持外部行为不变，以现有测试和静态检查作为回归保护。

**Tech Stack:** Rust workspace, clap, anyhow, rusqlite, serde, tokio

---

### Task 1: Split Root Application Modules

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/commands.rs`
- Create: `src/cli/output/mod.rs`
- Create: `src/cli/output/common.rs`
- Create: `src/cli/output/memory.rs`
- Create: `src/cli/output/system.rs`
- Modify: `src/config/mod.rs`
- Create: `src/config/app_home.rs`
- Create: `src/config/file_config.rs`
- Create: `src/config/provider_config.rs`
- Create: `src/config/templates.rs`
- Modify: `src/providers/adapters/mod.rs`
- Delete: `src/providers/adapters/extraction.rs`
- Create: `src/providers/adapters/extraction/mod.rs`
- Create: `src/providers/adapters/extraction/adapter.rs`
- Create: `src/providers/adapters/extraction/prompt.rs`
- Create: `src/providers/adapters/extraction/normalize.rs`

- [ ] 先把 `render/config/extraction` 拆成子模块，同时保持旧调用点不变。
- [ ] 用已有单元测试保护输出格式、配置解析与 extraction 清洗逻辑。

### Task 2: Split Engine DB Internals

**Files:**
- Modify: `crates/engine/src/db/mod.rs`
- Create: `crates/engine/src/db/index_jobs.rs`
- Create: `crates/engine/src/db/mappers.rs`
- Create: `crates/engine/src/db/schema.rs`
- Create: `crates/engine/src/db/support.rs`
- Create: `crates/engine/src/db/tests.rs`

- [ ] 把 `db/mod.rs` 中的基础设施代码拆出，保留 `Database` 公开方法与调用方式。
- [ ] 先迁移纯 helper 和 schema，再迁移测试，避免一上来切 public methods 导致大面积编译错误。

### Task 3: Verify And Commit

**Files:**
- Modify: `docs/superpowers/specs/2026-04-23-source-layout-refactor-design.md`
- Modify: `docs/superpowers/plans/2026-04-23-source-layout-refactor.md`

- [ ] 运行 `cargo test --all-targets --all-features`。
- [ ] 运行 `cargo fmt --all`。
- [ ] 运行 `cargo clippy --all-targets --all-features -- -D warnings`。
- [ ] 按仓库历史风格提交一个“源码结构收敛”提交，提交信息带 body 说明本次边界变化。
