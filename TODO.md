# Memo CLI - TODO 优化计划

> 本文档记录了项目的后续优化和功能扩展计划
> 
> 最后更新：2026-01-27

## 📊 进度概览

- 🔥 P0（架构修复）：已完成 ✅
- 🟡 P1（代码质量）：进行中 🚧
- 🟢 P2（功能扩展）：规划中 📋

---

## 🧪 测试相关

### 添加单元测试
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

**目标**：为核心模块编写单元测试，确保代码质量

**任务清单**：
- [ ] `parser/markdown.rs` - 测试 Markdown 解析功能
  - [ ] 测试 frontmatter 提取
  - [ ] 测试标题分段
  - [ ] 测试标签解析
- [ ] `models/memory.rs` - 测试数据模型
  - [ ] 测试 Memory 创建
  - [ ] 测试 MemoryBuilder
- [ ] `config.rs` - 测试配置加载逻辑
  - [ ] 测试本地/全局配置优先级
  - [ ] 测试配置文件解析

**预期成果**：
- 单元测试覆盖率 > 70%
- 核心功能都有测试保护

---

### 添加集成测试
**优先级**：⭐⭐  
**状态**：📋 待开始

**目标**：为 service 层编写集成测试，验证端到端功能

**任务清单**：
- [ ] `service/embed.rs` - 测试嵌入功能
  - [ ] 测试文本嵌入
  - [ ] 测试文件嵌入
  - [ ] 测试目录扫描和批量嵌入
- [ ] `service/search.rs` - 测试搜索功能
  - [ ] 测试语义搜索
  - [ ] 测试相似度阈值过滤
- [ ] `service/list.rs` - 测试列表功能
- [ ] `service/clear.rs` - 测试清空功能

**技术方案**：
- 使用临时目录创建测试数据库
- 测试结束后自动清理

---

## 🔧 代码质量提升

### 使用 thiserror 定义自定义错误类型
**优先级**：⭐⭐⭐⭐⭐  
**状态**：📋 优先待办

**目标**：创建清晰的错误类型体系，提升错误处理能力

**实现方案**：
```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoError {
    #[error("数据库未初始化: {0}")]
    DatabaseNotInitialized(String),
    
    #[error("模型加载失败: {0}")]
    ModelLoadError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("文件解析错误: {0}")]
    ParseError(String),
    
    #[error("数据库操作失败: {0}")]
    DatabaseError(String),
    
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, MemoError>;
```

**任务清单**：
- [ ] 在 types crate 创建 `error.rs` 文件，定义基础错误类型
- [ ] 在 local crate 定义存储层错误类型
- [ ] 在 cli crate 定义应用层错误类型
- [ ] 逐步将关键模块的 `anyhow::Result` 替换为具体错误类型
- [ ] 为不同错误类型提供更友好的用户提示

**预期收益**：
- 错误类型清晰，便于针对性处理
- 测试时可以精确断言错误类型
- 更好的用户体验（清晰的错误提示）

---

### 改进 Markdown 解析器
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

**目标**：使用成熟的 Markdown 解析库，支持更多功能

**问题分析**：
- 当前使用正则表达式解析，只支持 `##` 和 `###` 标题
- 无法正确处理代码块中的假标题
- 不支持 `#` 一级标题

**技术方案**：
```toml
# Cargo.toml
[dependencies]
pulldown-cmark = "0.11"
```

**实现要点**：
- 使用 `pulldown-cmark::Parser` 解析 Markdown
- 正确识别所有级别的标题（# 到 ######）
- 跳过代码块中的内容
- 保持对 frontmatter 的支持

**任务清单**：
- [ ] 添加 `pulldown-cmark` 依赖
- [ ] 重写 `parser/markdown.rs`
- [ ] 添加针对边界情况的测试
- [ ] 更新文档

---

## ⚡ 性能优化

### 批量嵌入性能优化
**优先级**：⭐⭐⭐⭐⭐  
**状态**：📋 优先待办

**目标**：使用并行处理提升批量嵌入性能

**技术方案**：
```toml
# Cargo.toml
[dependencies]
rayon = "1.8"
```

**优化方案**：
1. **文件扫描并行化**：使用 `rayon` 并行扫描目录
2. **批量向量化**：一次处理多个文本，减少模型调用次数
3. **数据库批量写入**：累积多条记录后一次性写入

**预期提升**：
- 文件扫描速度提升 2-3 倍
- 整体嵌入速度提升 30-50%

**任务清单**：
- [ ] 添加 `rayon` 依赖到 cli crate
- [ ] 使用 `par_bridge()` 并行扫描和解析文件
- [ ] 批量调用 embedding API（减少网络往返）
- [ ] 使用 `insert_batch` 批量写入数据库
- [ ] 添加进度显示（处理大量文件时）
- [ ] 添加性能基准测试（对比优化前后）

**预期收益**：
- 文件扫描速度提升 2-3 倍
- 整体嵌入速度提升 30-50%
- 更好的用户体验（进度显示）

---

## 🚀 功能扩展

### 支持标签过滤搜索
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

**目标**：在搜索和列表时支持按标签过滤

**命令设计**：
```bash
# 搜索时过滤标签
memo search "Vue" --tags "learning,vue"

# 列表时过滤标签
memo list --tags "rust"

# 支持标签组合（AND/OR）
memo search "最佳实践" --tags "vue+react"  # OR
memo search "最佳实践" --tags-all "vue,learning"  # AND
```

**任务清单**：
- [ ] 在 `Search` 和 `List` 命令添加 `--tags` 参数
- [ ] 实现标签过滤逻辑
- [ ] 支持 AND/OR 逻辑
- [ ] 更新搜索查询构建
- [ ] 添加标签统计功能

---

### 支持数据导出/导入
**优先级**：⭐⭐  
**状态**：📋 待开始

**目标**：支持数据备份和迁移

**命令设计**：
```bash
# 导出为 JSON
memo export --output backup.json

# 从 JSON 导入
memo import backup.json

# 导出特定标签
memo export --tags "important" --output important.json
```

**数据格式**：
```json
{
  "version": "0.1.0",
  "exported_at": "2026-01-22T12:00:00Z",
  "memories": [
    {
      "id": "uuid",
      "title": "...",
      "content": "...",
      "tags": ["tag1", "tag2"],
      "created_at": "2026-01-20T10:00:00Z",
      "updated_at": "2026-01-22T11:00:00Z"
    }
  ]
}
```

**任务清单**：
- [ ] 在 `cli.rs` 添加 `Export` 和 `Import` 命令
- [ ] 实现 `service/export.rs`
- [ ] 实现 `service/import.rs`
- [ ] 设计导出数据格式（JSON）
- [ ] 支持增量导入（跳过已存在）
- [ ] 添加数据验证

---

## 🔄 工程化

### 添加 GitHub Actions CI/CD
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

**目标**：实现自动化测试和发布

**配置文件**：`.github/workflows/ci.yml`

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - run: cargo build --release

  release:
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    needs: test
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: softprops/action-gh-release@v1
```

**任务清单**：
- [ ] 创建 `.github/workflows/ci.yml`
- [ ] 配置自动格式化检查
- [ ] 配置 Clippy 检查
- [ ] 配置测试运行
- [ ] 配置发布构建
- [ ] 添加多平台构建（Windows/macOS/Linux）
- [ ] 配置版本发布自动化

---

## 📈 进度跟踪

### 已完成 ✅

#### 架构修复（2026-01-27）
- ✅ **重构配置加载逻辑** - 提取 ConfigLoader 模式，消除重复代码
- ✅ **修复向量维度管理** - 添加 metadata.json，连接时验证维度匹配
- ✅ **扩展重复检测** - 提取通用函数，应用到所有嵌入路径
- ✅ **修复 update 操作数据安全** - 先插入后删除，添加错误恢复机制
- ✅ **修复 merge 操作保留 created_at** - 保留最早的创建时间

#### 早期修复（2026-01-22）
- ✅ 删除未使用的 `embedding/api.rs`
- ✅ 修复 Runtime 重复创建
- ✅ 修正搜索相似度计算
- ✅ 移除 service 内部的 Runtime
- ✅ 删除未使用的 utils 模块
- ✅ 改进模块组织（添加分组注释）
- ✅ 初始化日志系统

### 进行中 🚧

（暂无）

### 优先待办 ⭐

#### P1 - 代码质量提升
- **使用 thiserror 定义自定义错误类型**（见下文）
- **批量嵌入性能优化**（见下文）

#### P2 - 功能完善
- 标签过滤搜索（见下文）
- 改进 Markdown 解析器（见下文）
- 添加单元测试（见下文）

其他任务详见各章节（共 9 个任务）

### 已删除 🗑️

以下任务已被删除（已完成或不适用）：
- ✅ CLI 输出格式优化（已完成）
- ❌ 错误信息中文化（当前英文信息已足够清晰）
- ❌ 模型加载优化（不适用：项目使用 API 而非本地模型）
- ❌ 文本标准化策略（当前方案已是最佳实践）
- ❌ 三层存储结构（基于错误假设：数据模型无 summary 字段）

---

## 🎯 推荐实施顺序

### ✅ 第零阶段：核心痛点解决（已完成）
- **智能记忆整合系统** - 解决重复记忆问题 ✅

### 第一阶段：基础质量提升
- **添加单元测试** - 保证代码质量
- **自定义错误类型** - 改善错误处理

### 第二阶段：核心功能完善
- **标签过滤搜索** - 高频使用功能
- **改进 Markdown 解析器** - 提升解析能力
- **渐进式搜索策略** - 改善搜索体验

### 第三阶段：性能优化
- **批量嵌入性能优化** - 提升大文件处理速度

### 第四阶段：工程化完善
- **添加集成测试** - 完整的测试覆盖
- **CI/CD 流程** - 自动化构建和发布

### 第五阶段：功能扩展（按需）
- **数据导出/导入**

---

## 🎨 用户体验优化

### 实现渐进式搜索策略
**优先级**：⭐⭐  
**状态**：📋 待开始

**目标**：当搜索无结果时自动降低阈值重试

**用户痛点**：
- 默认阈值 0.7 过于严格，可能找不到相关结果
- 用户需要手动调整 `--threshold` 参数

**改进方案**：
```rust
// 渐进式搜索
pub async fn search_with_fallback(query: &str, limit: usize) -> Result<Vec<Memory>> {
    let thresholds = [0.7, 0.6, 0.5];
    
    for threshold in thresholds {
        let results = search(query, limit, threshold).await?;
        
        if !results.is_empty() {
            eprintln!("  使用阈值: {}", threshold);
            return Ok(results);
        }
    }
    
    Ok(vec![])
}
```

**优化点**：
- 默认从 0.7 开始（精确匹配）
- 无结果时降至 0.6（模糊匹配）
- 仍无结果降至 0.5（宽泛搜索）
- 提示用户当前使用的阈值

**任务清单**：
- [ ] 实现 `search_with_fallback` 函数
- [ ] 添加 `--strict` 参数禁用降级
- [ ] 在输出中显示使用的阈值
- [ ] 结合时间过滤和标签提高准确性

---

## 📝 更新日志

- **2026-01-27**: 完成架构修复，更新 TODO 文档
  - ✅ 完成 5 个核心架构问题修复
  - ✅ 清理已完成任务，重新组织优先级
  - ✅ 标记优先待办：thiserror 错误处理、批量嵌入性能优化
- **2026-01-26**: 彻底清理已完成和过期任务
  - 🗑️ 删除已完成任务：CLI 输出格式优化
  - 🗑️ 删除过期任务：错误信息中文化、模型加载优化、文本标准化策略、三层存储结构
- **2026-01-25**: 添加用户体验优化相关任务
- **2026-01-22**: 创建 TODO 文档，完成早期问题修复

---

## 🤝 贡献指南

如果您想参与某个 TODO 项的开发：

1. 在对应任务的 Issue 中留言
2. Fork 项目并创建分支
3. 完成开发并添加测试
4. 提交 Pull Request

期待您的贡献！🎉
