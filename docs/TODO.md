# Memo CLI - TODO 优化计划

> 本文档记录了项目的后续优化和功能扩展计划
> 
> 最后更新：2026-01-22

## 📊 进度概览

- 🔥 P0（严重问题）：已完成 ✅
- 🟡 P1（中等问题）：已完成 ✅
- 🟢 P2（可选优化）：进行中 🚧

---

## 🧪 测试相关

### 1. 添加单元测试
**优先级**：⭐⭐⭐⭐  
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

### 2. 添加集成测试
**优先级**：⭐⭐⭐  
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

### 3. 使用 thiserror 定义自定义错误类型
**优先级**：⭐⭐⭐⭐  
**状态**：📋 待开始

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
- [ ] 创建 `src/error.rs` 文件
- [ ] 定义 `MemoError` 枚举
- [ ] 将所有 `anyhow::Result` 替换为 `crate::error::Result`
- [ ] 在各模块中使用具体的错误类型

---

### 4. 改进 Markdown 解析器
**优先级**：⭐⭐⭐⭐  
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

### 5. 错误信息中文化
**优先级**：⭐⭐⭐⭐⭐  
**状态**：📋 待开始

**目标**：将所有错误提示改为中文，提升用户体验

**任务清单**：
- [ ] 审查所有 `.context()` 和 `.with_context()` 错误信息
- [ ] 将英文错误信息翻译为中文
- [ ] 确保错误信息清晰、友好
- [ ] 提供可操作的建议

**示例**：
```rust
// 修改前
.context("Failed to read config file")?

// 修改后
.context("读取配置文件失败，请检查文件权限")?
```

---

## ⚡ 性能优化

### 6. 批量嵌入性能优化
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

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
- [ ] 添加 `rayon` 依赖
- [ ] 使用 `par_bridge()` 并行处理文件
- [ ] 实现批量向量化接口
- [ ] 优化数据库写入策略
- [ ] 添加性能基准测试

---

### 7. 模型加载优化
**优先级**：⭐⭐  
**状态**：📋 待开始

**目标**：避免重复加载模型（500MB），提升响应速度

**问题分析**：
- 每次命令都会加载模型，即使连续执行多次
- 模型加载耗时 5-10 秒

**方案对比**：

| 方案 | 优点 | 缺点 | 难度 |
|------|------|------|------|
| Daemon 服务 | 最优性能，响应快 | 需要进程管理 | 高 |
| Lazy 全局缓存 | 简单实用 | 只在单次运行有效 | 低 |
| 模型量化 | 减少内存占用 | 可能影响精度 | 中 |

**推荐方案**：先实现 Lazy 缓存，长期考虑 Daemon

**任务清单**：
- [ ] 短期：使用 `once_cell::sync::Lazy` 实现全局缓存
- [ ] 长期：设计 daemon 服务架构
- [ ] 长期：实现客户端-服务端通信
- [ ] 长期：添加模型预热机制

---

## 🚀 功能扩展

### 8. 支持更新单条记录
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

**目标**：添加 `memo update` 命令，支持修改已有记录

**命令设计**：
```bash
# 通过 ID 更新
memo update <id> --content "新内容"

# 通过搜索更新
memo update --search "查询" --content "新内容"

# 更新标签
memo update <id> --tags "tag1,tag2"
```

**任务清单**：
- [ ] 在 `cli.rs` 添加 `Update` 命令
- [ ] 实现 `service/update.rs`
- [ ] 支持按 ID 查找
- [ ] 支持按标题/内容搜索
- [ ] 重新计算向量嵌入
- [ ] 更新数据库记录

---

### 9. 支持删除单条记录
**优先级**：⭐⭐⭐  
**状态**：📋 待开始

**目标**：添加 `memo delete` 命令，支持删除指定记录

**命令设计**：
```bash
# 通过 ID 删除
memo delete <id>

# 通过搜索删除（交互式确认）
memo delete --search "查询"

# 批量删除（需要确认）
memo delete --all --tag "临时"
```

**任务清单**：
- [ ] 在 `cli.rs` 添加 `Delete` 命令
- [ ] 实现 `service/delete.rs`
- [ ] 支持按 ID 删除
- [ ] 支持按条件删除
- [ ] 添加确认提示（防止误删）
- [ ] 显示被删除记录的信息

---

### 10. 支持标签过滤搜索
**优先级**：⭐⭐⭐⭐  
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

### 11. 支持数据导出/导入
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

### 12. 添加 GitHub Actions CI/CD
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

#### P0 - 严重问题
- ✅ 删除未使用的 `embedding/api.rs`
- ✅ 修复 Runtime 重复创建
- ✅ 修正搜索相似度计算
- ✅ 移除 service 内部的 Runtime

#### P1 - 中等问题
- ✅ 删除未使用的 utils 模块
- ✅ 改进模块组织（添加分组注释）
- ✅ 初始化日志系统
- ✅ 简化配置加载逻辑

### 进行中 🚧

（暂无）

### 待开始 📋

12 个 P2 优化任务（详见上文）

---

## 🎯 推荐实施顺序

### 第一阶段：基础质量提升（1-2 周）
1. **错误信息中文化** - 快速提升用户体验
2. **添加单元测试** - 保证代码质量
3. **自定义错误类型** - 改善错误处理

### 第二阶段：核心功能完善（2-3 周）
4. **标签过滤搜索** - 高频使用功能
5. **支持删除记录** - 完善 CRUD
6. **改进 Markdown 解析器** - 提升解析能力

### 第三阶段：性能优化（1-2 周）
7. **批量嵌入性能优化** - 提升大文件处理速度
8. **模型加载优化** - 改善启动速度

### 第四阶段：工程化完善（1 周）
9. **添加集成测试** - 完整的测试覆盖
10. **CI/CD 流程** - 自动化构建和发布

### 第五阶段：功能扩展（按需）
11. **更新记录功能**
12. **数据导出/导入**

---

## 🎨 用户体验优化

### 13. 优化文本标准化策略
**优先级**：⭐⭐⭐  
**状态**：💭 待讨论

**目标**：统一嵌入向量和存储内容的文本处理策略

**当前实现**：
- 向量嵌入使用标准化文本（`normalize_for_embedding`）
- `summary` 和 `content` 字段存储原始文本
- 这种不一致可能影响搜索体验

**方案对比**：

| 方案 | 优点 | 缺点 | 推荐 |
|------|------|------|------|
| 全部标准化 | 搜索更一致，节省 token | 丢失原始格式信息 | ❌ |
| 分离存储 | 向量准确，内容完整 | 当前方案 | ✅ |
| 双份存储 | 兼顾两者 | 占用空间更大 | 🤔 |

**讨论要点**：
- 标准化是否影响嵌入质量？（实验结果：影响很小，0.86 vs 0.90）
- 用户是否需要保留原始格式？
- 搜索一致性 vs 内容完整性的权衡

**任务清单**：
- [ ] 收集更多搜索场景数据
- [ ] 评估标准化对嵌入质量的影响
- [ ] 根据实际使用反馈决定最终方案

---

### 14. 充分利用三层存储结构
**优先级**：⭐⭐  
**状态**：📋 待开始

**目标**：优化 `title`/`summary`/`content` 的展示和使用

**当前状态**：
- `title`: 50 字符，用于列表展示 ✅
- `summary`: 200 字符，用于预览 ✅
- `content`: 完整原文，**基本未使用** ❌

**改进方案**：

1. **添加详细查看命令**
   ```bash
   memo show <id>  # 展示完整 content
   memo show --search "查询"  # 搜索后查看详情
   ```

2. **在搜索结果中提供详情选项**
   ```
   [0.90] Rust 向量数据库实现
       Rust 向量数据库实现 - LanceDB 集成经验...
       
   > 按 Enter 查看完整内容，q 退出
   ```

3. **支持导出为 Markdown**
   ```bash
   memo export <id> --format md > output.md
   ```

**任务清单**：
- [ ] 实现 `memo show <id>` 命令
- [ ] 在 `search` 结果中添加交互式详情查看
- [ ] 支持导出单条记录为 Markdown
- [ ] 优化 `content` 的显示格式（语法高亮、分页）

---

### 15. 优化 CLI 输出格式
**优先级**：⭐⭐⭐⭐  
**状态**：✅ 部分完成

**目标**：改进命令行输出的可读性和信息密度

**已完成**：
- ✅ 调整状态消息缩进（统一使用 7 空格）
- ✅ 优化数据库信息展示（添加模型维度）
- ✅ `list` 输出的 summary 截断到 100 字符
- ✅ 过滤 summary 中的换行符
- ✅ 使用灰色字体显示 summary

**待优化**：
- [ ] 添加颜色主题配置（支持自定义颜色）
- [ ] 支持 `--format` 参数（json/table/compact）
- [ ] 优化长标题的省略显示
- [ ] 添加进度条（批量嵌入时）

**当前输出示例**：
```
   Database: ~/.memo/brain (6 records)
      Model: text-embedding-v4 (1024d)

[1/6] Rust 向量数据库实现 - LanceDB 集成经验 (2026-01-25 12:26)
    Rust 向量数据库实现 - LanceDB 集成经验 背景：需要在 Rust CLI 中实现语义搜索功能 方案：使用 LanceDB...
```

---

### 16. 实现渐进式搜索策略
**优先级**：⭐⭐⭐  
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

- **2026-01-22**: 创建 TODO 文档，完成 P0 和 P1 问题修复
- **2026-01-25**: 添加用户体验优化相关任务（文本标准化、三层结构、CLI 输出、渐进式搜索）
- 后续更新将记录在此...

---

## 🤝 贡献指南

如果您想参与某个 TODO 项的开发：

1. 在对应任务的 Issue 中留言
2. Fork 项目并创建分支
3. 完成开发并添加测试
4. 提交 Pull Request

期待您的贡献！🎉
