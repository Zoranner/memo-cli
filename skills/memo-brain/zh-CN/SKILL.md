---
name: memo-brain
description: 管理和检索跨对话的上下文记忆，使用向量数据库记录经验、解决方案和用户习惯。通过语义搜索提供上下文感知的协助。当需要记住信息、搜索记忆、或用户明确要求"记住这个"、"查记忆"时使用。
---

# Memo Brain 记忆管理

使用向量数据库语义搜索记录和检索有价值的知识。

**⚠️ 重要：所有命令需要网络访问权限（调用嵌入 API）。使用 Shell 工具时，设置 `required_permissions: ["network"]` 或 `["all"]`。**

## 命令

| 命令 | 用途 | 示例 |
|---------|---------|---------|
| `memo embed <text>` | 记录记忆（仅文本） | `memo embed "背景：... 方案：..." --tags rust,cli` |
| `memo search <query>` | 搜索记忆 | `memo search "如何使用 rust async" -n 5` |
| `memo list` | 列出所有记忆 | `memo list` |
| `memo update <id>` | 更新记忆 | `memo update abc123 --content "新内容" --tags rust,async` |
| `memo merge <ids>...` | 合并记忆 | `memo merge id1 id2 --content "合并内容" --tags rust,cli` |
| `memo delete <id>` | 删除记忆 | `memo delete abc123 --force` |

**重复检测：** 嵌入时如果检测到相似内容（相似度 > 0.85）会提示，应优先考虑合并或更新，避免盲目创建新记忆。

## 常用参数

| 参数 | 用途 | 默认值 |
|-----------|---------|---------|
| `-t, --tags` | 添加标签（逗号分隔） | - |
| `-n, --limit` | 搜索结果数量 | 5 |
| `--threshold` | 相似度阈值（0-1） | 0.7 |
| `--after / --before` | 时间过滤（YYYY-MM-DD） | - |

---

## 核心工作流程

### 何时记录

**应该记录：**
- 解决了复杂问题或发现巧妙方案
- 用户明确要求（"记住这个"、"记录下来"）
- 工具配置、用户偏好等有价值的经验

**不应该记录：**
- 简单语法查询、常识性知识、临时方案
- 重复内容（**先搜索现有记忆**，优先合并/更新）

### 何时搜索

**触发场景：**
- 遇到类似的问题或任务（"之前怎么解决的"）
- 用户明确要求（"查记忆"、"还记得吗"）
- 开始新任务前检查相关经验
- 查找最近的工作（使用 `--after`）

**搜索原则：**
- ✅ 使用完整、具体的问题句式，包含技术点、场景、问题等详细信息
- ❌ 不要只罗列 2-3 个关键词（如 "rust async trait"）
- 复杂请求拆解成 2-3 个子问题分别搜索
- 越详细越好：具体的技术栈、具体的使用场景、具体的问题描述

### 处理重复记忆

当 `memo embed` 检测到相似记忆时（相似度 > 0.85），系统会自动显示相似记忆的完整信息（ID、内容、分数、日期）。**不要直接跳过或强制添加**，应该根据显示的信息评估并决策：

**决策流程：**
```bash
# 1. 查看系统显示的相似记忆信息（已包含完整内容）

# 2. 评估并决策：
#    - 新内容是对原有内容的细化补充 → 使用 update
#    - 多条记忆有重叠且可以整合 → 使用 merge

# 示例 A：更新现有记忆（补充细节）
memo update abc123 --content "原有内容 + 新的细节和补充" --tags rust,async

# 示例 B：合并相似记忆（整合重叠内容）
memo merge id1 id2 --content "整合后的完整内容，涵盖两条记忆的要点" --tags rust,error-handling

# 示例 C：删除后重新嵌入（内容完全替代）
memo delete abc123 --force
memo embed "全新的完整内容..." --tags rust,optimization
```

**决策优先级：**
1. ✅ **合并** - 内容有重叠且可以整合为一条清晰完整的记忆（**注意粒度控制，不要盲目合并**）
2. ✅ **更新** - 新内容是对现有记忆的补充和细化（**注意记忆不要过大**）
3. ✅ **拆分** - 现有记忆已经过大或包含多个独立主题（**保持每条记忆聚焦单一主题**）
4. ✅ **新增** - 确认是完全独立的新知识点

**记忆粒度控制原则：**
- ✅ **合适大小**：每条记忆 100-300 字，不超过 500 字
- ✅ **单一主题**：每条记忆聚焦一个核心主题或问题
- ✅ **清晰结构**：背景、方案、关键点分明，易于快速理解
- ❌ **过度合并**：不要把不同场景、不同问题硬凑到一起
- ❌ **过度细碎**：不要把紧密相关的内容拆得过细

**何时应该拆分记忆：**
- 单条记忆超过 500 字，包含多个独立要点
- 记忆涵盖多个不同的使用场景或问题
- 记忆混合了多个技术栈或概念
- 搜索时经常只需要其中某一部分内容

---

## 内容格式

**模板：**
```
[主题] - [简短标题]

背景：[1-2 句话描述上下文]
方案：[具体解决方案或知识]
关键点：[关键要点或注意事项]
```

**示例：**

```bash
memo embed "Rust async trait - 使用 async-trait crate

背景：在 trait 中直接使用 async fn 会导致编译错误
方案：在 trait 和 impl 上使用 #[async_trait] 宏
关键点：trait 定义和 impl 都需要添加该宏" --tags rust,async
```

**注意：** 使用双引号包裹多行文本。在 Windows CMD 中使用 `"`，在 PowerShell/Bash 中转义内部引号或使用 heredoc。

---

## 最佳实践

### 内容质量

| 指南 | 描述 |
|-----------|-------------|
| 简洁 | 每条记忆 100-300 字，最多不超过 500 字 |
| 单一主题 | 每条记忆聚焦一个核心问题或主题 |
| 结构化 | 使用一致的模板（背景、方案、关键点） |
| 精确标签 | 准确分类，便于后续过滤 |
| 适当粒度 | 过大应拆分，过碎应合并 |

### 标签策略

```bash
# 按技术分类
memo embed "..." --tags vue,frontend

# 按重要性分类  
memo embed "..." --tags important,decision

# 按项目分类
memo embed "..." --tags project-x,config

# 按类型分类
memo embed "..." --tags security,bug-fix
```

### 常见错误

| ❌ 不要 | ✅ 应该 |
|----------|--------|
| 记录整个代码文件 | 只提取关键片段 |
| 记录每个答案 | 只记录有价值的见解 |
| 不检查就记录 | 先搜索避免重复 |
| 遇到相似记忆直接跳过 | 优先考虑合并或更新现有记忆 |
| 使用模糊标题 | 具体且描述性强 |
| 太多通用标签 | 保持标签集中 |
| `memo search "rust async"` | `memo search "如何在 Rust trait 中使用 async 函数"` |

---

## 使用示例

### 带标签记录

```bash
memo embed "Rust 错误处理 - 应用层使用 anyhow

背景：应用程序代码需要简单的错误传播机制
方案：使用 anyhow::Result 作为返回类型，用 ? 进行错误传播
关键点：库使用 thiserror，应用使用 anyhow" --tags rust,error-handling
```

### 记忆管理

```bash
# 更新记忆
memo update abc123 --content "更新后的内容" --tags rust,async

# 删除记忆
memo delete abc123 --force

# 合并多个记忆
memo merge id1 id2 id3 --content "合并后的总结内容" --tags rust,cli
```

### 搜索示例

**简单问题：**
```bash
# 好的做法：完整问题
memo search "如何在 Rust trait 中使用 async 函数" -n 5

# 不推荐：只用关键词
memo search "rust async trait" -n 5
```

**复杂请求分解：**
```bash
# 用户请求："我想构建一个 Rust Web API，需要处理异步请求和数据库连接"
# 建议分解为 3 个子问题：

memo search "如何在 Rust 中构建 Web API 服务" -n 5
memo search "Rust 异步请求处理的最佳实践" -n 5
memo search "如何管理 Rust 数据库连接池" -n 5
```

**参数调整：**
```bash
# 获取更多结果
memo search "如何优化数据库查询性能" -n 10

# 降低阈值以获得更广泛的匹配
memo search "错误处理的最佳实践" --threshold 0.6

# 时间过滤
memo search "最近修复了哪些 bug" --after 2026-01-15

# 组合使用
memo search "Vue 组件有哪些常用模式" -n 10 --after 2026-01-01 --threshold 0.6
```

### 拆分记忆示例

```bash
# 场景：一条记忆包含"CLI 输出格式"的多个方面，已经 600 字
# 原记忆混合了：空行控制、错误处理、输出方法规范

# 拆分为 3 条独立记忆：
memo embed "CLI 输出格式 - 空行与分隔符控制..." --tags rust,cli,output
memo embed "CLI 错误退出 - exit vs bail 的选择..." --tags rust,cli,error
memo embed "CLI 输出规范 - 禁止直接使用 println..." --tags rust,cli,output

# 删除原来过大的记忆
memo delete original-id --force
```

### 基于时间的搜索

```bash
# 最近的工作
memo search "数据库优化相关的经验" --after 2026-01-15

# 特定时期
memo search "项目进展情况" --after 2026-01-01 --before 2026-01-31 -n 20
```

---

## 触发短语

| 操作 | 触发短语 |
|--------|---------|
| **记录** | "记住这个", "记录下来", "总结经验", "总结并记录", "保存这个" |
| **搜索** | "之前怎么做的", "查记忆", "还记得吗", "最近有没有...", "搜索记忆", "有没有类似的经验" |
