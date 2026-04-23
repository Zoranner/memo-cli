# Source Layout Refactor Design

**Date:** 2026-04-23

## Goal

把 `src` 与 `crates/engine/src` 从“旧单文件拆出了一半”的过渡态，收敛成一套更稳定的工程结构。目标是让后续维护首先看到职责边界，而不是先钻进大文件或历史遗留命名。

## Scope

本次只做结构性重构，不改变 CLI 命令语义、不改变 engine 的对外行为、不顺手扩功能。重点是目录边界、模块拆分、内部可维护性和可验证性。

## Design

### 根级 `src`

- `src/cli` 负责命令参数、命令分发、路径解析、输出渲染。
- `src/config` 负责应用初始化、配置文件解析、provider 配置解析与 engine 配置装配。
- `src/providers` 负责 provider 适配器、重试运行时与运行状态记录。

进一步拆分：

- `src/cli/render.rs` 拆成 `src/cli/output/*`，把通用 JSON 输出、memory 输出、system 输出分开。
- `src/config/mod.rs` 拆成 `app_home`、`file_config`、`provider_config`、`templates` 等子模块。
- `src/providers/adapters/extraction.rs` 拆成 `extraction/{adapter,prompt,normalize}`，让 prompt、调用、清洗归一化不再混在一个文件里。

### `crates/engine/src`

- `engine/` 继续承载 dream / ingest / recall / restore 等编排逻辑。
- `types/` 保持值对象与输入输出类型。
- `db/` 从巨型 `mod.rs` 继续拆出内部子模块，至少先把 schema、index jobs、row mappers、shared utils、tests 分离。
- `text_index.rs` / `vector_index.rs` 暂时保留文件级 API，不在本次重构里同时改为另一套 public path，避免范围失控。

## Constraints

- 迁移后 public API 与行为保持兼容。
- 现有测试必须继续通过。
- 完成后必须执行 `cargo fmt --all` 与 `cargo clippy --all-targets --all-features -- -D warnings`。

## Verification

- `cargo test --all-targets --all-features`
- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
