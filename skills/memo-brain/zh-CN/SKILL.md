---
name: memo-brain
description: 使用向量数据库 CLI 工具管理上下文记忆。记录对话中的宝贵经验、解决方案和用户习惯。通过语义搜索提供上下文感知的协助。用于跨对话记忆或检索信息，或当用户要求记住某事或搜索记忆时。
---

# Memo Brain 记忆管理

使用向量数据库语义搜索记录和检索有价值的知识。

## 命令

| 命令 | 用途 | 示例 |
|---------|---------|---------|
| `memo embed <text>` | 记录记忆（仅文本） | `memo embed "背景：... 方案：..." --tags rust,cli` |
| `memo search <query>` | 搜索记忆 | `memo search "如何使用 rust async" -n 5` |
| `memo list` | 列出所有记忆 | `memo list` |
| `memo update <id>` | 更新记忆 | `memo update abc123 --content "新内容"` |
| `memo merge <ids>...` | 合并记忆 | `memo merge id1 id2 --content "合并内容"` |
| `memo delete <id>` | 删除记忆 | `memo delete abc123` |

> **网络访问：** 这些命令需要网络访问以使用嵌入 API。使用 Shell 工具时，设置 `required_permissions: ["network"]` 或 `["all"]`。
>
> **重复检测：** 嵌入时如果检测到相似内容会有提示，根据提示选择后续操作即可。

## 常用参数

| 参数 | 用途 | 默认值 |
|-----------|---------|---------|
| `-t, --tags` | 添加标签（逗号分隔） | - |
| `-n, --limit` | 结果数量 | 5 |
| `--threshold` | 相似度阈值（0-1） | 0.7 |
| `--after` | 按时间过滤（YYYY-MM-DD 或 YYYY-MM-DD HH:MM） | - |
| `--before` | 按时间过滤（YYYY-MM-DD 或 YYYY-MM-DD HH:MM） | - |

---

## 何时记录

### ✅ 应该记录的情况

| 情况 | 示例 |
|-----------|---------|
| 解决了复杂问题 | 调试异步生命周期问题 |
| 发现巧妙解决方案 | 非显而易见的变通方法或模式 |
| 用户明确要求 | "remember this", "记住这个", "record this", "记录下来" |
| 工具/库配置 | 设置构建工具、API 配置 |
| 用户偏好 | 编码风格、库选择 |

### ❌ 不应该记录

- 简单的语法查询
- 常识性知识
- 临时性的变通方案
- 重复内容（先搜索）

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

## 何时搜索

### 触发场景

| 触发器 | 示例查询 |
|---------|---------------|
| 类似的过往问题 | "how did we solve", "之前怎么解决的" |
| 回忆特定解决方案 | "how to configure", "那个配置怎么写" |
| 用户明确要求 | "search memory", "查记忆", "do you remember", "还记得吗" |
| 开始新任务 | 检查相关经验 |
| 最近的工作 | "recently...", "最近有没有..." (使用 `--after`) |

### 搜索原则

向量数据库使用语义理解，能够识别完整问题的含义和上下文。为了获得最佳搜索效果：

1. **使用完整问题句式**：描述你想知道什么，而不是罗列关键词
2. **分解复杂请求**：遇到包含多个方面的复杂问题时，拆解成 2-3 个独立的子问题分别搜索
3. **包含必要上下文**：提供足够的背景信息帮助理解意图

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

# 时间过滤（使用实际日期）
memo search "最近修复了哪些 bug" --after YYYY-MM-DD

# 组合使用
memo search "Vue 组件有哪些常用模式" -n 10 --after YYYY-MM-DD --threshold 0.6
```

### 参数指南

| 参数 | 典型值 | 调整建议 |
|-----------|---------|----------------|
| `-n` | 5 | 复杂主题或需要全面了解时用 10-20 |
| `--threshold` | 0.7 | 无结果时降低到 0.6-0.5 获得更多候选 |
| `--after` | - | 查找最近工作或特定时间段的记忆 |

### 使用结果

1. 按相似度分数解析返回的记忆
2. 提取与当前上下文相关的部分
3. 引用来源：相似度分数、更新时间、标签

---

## 最佳实践

### 内容质量

| 指南 | 描述 |
|-----------|-------------|
| 简洁 | 每条记忆 100-300 字 |
| 结构化 | 使用一致的模板 |
| 精确标签 | 准确分类，便于后续过滤 |

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

### 搜索技巧

| 策略 | 说明 |
|----------|-------------|
| 完整问题优先 | 用自然语言提问，让向量搜索理解完整语义 |
| 分解复杂请求 | 多方面的问题拆解成 2-3 个子问题分别搜索 |
| 包含上下文 | 描述问题场景和背景提高匹配准确度 |
| 渐进式搜索 | 从完整问题开始，无结果时简化或降低阈值 |
| 时间过滤 | 利用 `--after`/`--before` 定位特定时期的记忆 |

### 常见错误

| ❌ 不要 | ✅ 应该 |
|----------|--------|
| 记录整个代码文件 | 只提取关键片段 |
| 记录每个答案 | 只记录有价值的见解 |
| 不检查就记录 | 先搜索避免重复 |
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
memo update abc123 --content "更新后的内容" --tags rust,updated

# 删除记忆
memo delete abc123

# 合并多个记忆
memo merge id1 id2 id3 --content "合并后的总结内容"
```

### 基于时间的搜索

```bash
# 最近的工作（使用实际日期）
memo search "数据库优化相关的经验" --after 2026-01-15

# 特定时期
memo search "项目进展情况" --after 2026-01-01 --before 2026-01-31 -n 20
```

## 触发短语

| 操作 | 触发短语 |
|--------|---------|
| **记录** | "记住这个", "记录下来", "总结经验", "总结并记录", "保存这个" |
| **搜索** | "之前怎么做的", "查记忆", "还记得吗", "最近有没有...", "搜索记忆", "有没有类似的经验" |
