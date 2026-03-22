# Memo CLI - TODO 优化计划

> 本文档记录了项目的后续优化和功能扩展计划
> 
> 最后更新：2026-03-22

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
  - [ ] 测试重复检测
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
**优先级**：⭐⭐⭐  
**状态**：🚧 部分完成

**目标**：创建清晰的错误类型体系，提升错误处理能力

**已完成**：
- ✅ `crates/providers/src/error.rs` - Provider 错误类型
- ✅ `crates/local` - 存储层错误类型（依赖 thiserror）

**待完成**：
- [ ] 在 types crate 创建 `error.rs` 文件，定义基础错误类型
- [ ] 在 cli crate 定义应用层错误类型
- [ ] 逐步将关键模块的 `anyhow::Result` 替换为具体错误类型
- [ ] 为不同错误类型提供更友好的用户提示

**预期收益**：
- 错误类型清晰，便于针对性处理
- 测试时可以精确断言错误类型
- 更好的用户体验（清晰的错误提示）

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

## 🎨 用户体验优化

### 实现渐进式搜索策略
**优先级**：⭐⭐  
**状态**：📋 待开始

**目标**：当搜索无结果时自动降低阈值重试

**当前状态**：
- 默认阈值 0.35 已相对宽松
- 搜索无结果时会提示用户降低阈值

**改进方案**：
```rust
// 渐进式搜索
pub async fn search_with_fallback(query: &str, limit: usize) -> Result<Vec<Memory>> {
    let thresholds = [0.35, 0.25, 0.15];
    
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

**任务清单**：
- [ ] 实现 `search_with_fallback` 函数
- [ ] 添加 `--strict` 参数禁用降级
- [ ] 在输出中显示使用的阈值
- [ ] 结合时间过滤和标签提高准确性

---

## 📈 进度跟踪

### 已完成 ✅

#### 架构简化（2026-03-22）
- ✅ **移除 Markdown 解析功能** - 简化核心定位
  - 删除 `src/parser/` 目录
  - 删除 `MemoSection` 和 `MemoMetadata` 类型
  - 移除 `walkdir`、`shellexpand`、`regex` 依赖
  - `embed` 命令现在只支持纯文本输入

#### 工程化（2026-03-22）
- ✅ **GitHub Actions CI/CD** - 完整的自动化测试和发布流程
  - fmt/clippy/test 检查
  - 多平台构建（Windows/Linux glibc/Linux musl/macOS Intel/macOS ARM）
  - 自动发布到 GitHub Releases

#### 代码质量（部分完成）
- ✅ **thiserror 错误类型** - crates/providers 和 crates/local 已实现

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

- **thiserror 错误类型** - 需要扩展到 cli crate 和 types crate

### 优先待办 ⭐

#### P1 - 功能完善
- 标签过滤搜索
- 自定义错误类型（完成剩余部分）
- 添加单元测试

#### P2 - 按需实现
- 渐进式搜索策略
- 数据导出/导入
- 集成测试

---

## 🎯 推荐实施顺序

### ✅ 第零阶段：核心痛点解决（已完成）
- **智能记忆整合系统** - 解决重复记忆问题 ✅

### ✅ 第一阶段：工程化基础（已完成）
- **GitHub Actions CI/CD** - 自动化测试和发布 ✅

### 第二阶段：核心功能完善
- **标签过滤搜索** - 高频使用功能
- **自定义错误类型** - 完成剩余部分

### 第三阶段：质量保障
- **添加单元测试** - 保证代码质量
- **添加集成测试** - 完整的测试覆盖

### 第四阶段：功能扩展（按需）
- **渐进式搜索策略** - 改善搜索体验
- **数据导出/导入** - 数据备份和迁移

---

## 📝 更新日志

- **2026-03-22**: 移除 Markdown 解析功能，简化架构
  - 🗑️ 删除 `src/parser/` 目录及相关类型
  - 🗑️ 移除 `walkdir`、`shellexpand`、`regex` 依赖
  - 🗑️ 删除「改进 Markdown 解析器」任务
  - 🗑️ 删除「批量嵌入性能优化」任务（不再需要文件扫描）
  - 📋 更新单元测试任务清单
- **2026-03-22**: 重新审视 TODO 文档，修正漂移状态
  - ✅ GitHub Actions CI/CD 已完成，更新状态
  - 🚧 thiserror 错误类型部分完成，更新任务清单
  - 🔧 修正渐进式搜索策略描述（默认阈值 0.35）
  - 📋 重新组织优先级和实施顺序
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

期待您的贡献！
