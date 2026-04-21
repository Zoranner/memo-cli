---
name: memo-brain
description: 管理和检索跨对话的上下文记忆。公开动作语义以 command-philosophy 为准。适用于“记住这个”“查记忆”“看看当前状态”“恢复索引”等场景。
---

# Memo Brain 记忆管理

这个 skill 的动作语义，以 `docs/architecture/command-philosophy.md` 为准。

## 标准动作

- `memo awaken`
- `memo remember`
- `memo recall`
- `memo reflect`
- `memo dream`
- `memo state`
- `memo restore`

## 当前能力边界

当前 CLI **没有**这些旧接口，禁止再按它们思考或执行：

- `memo embed`
- `memo search`
- `memo update`
- `memo merge`
- `memo delete`
- `memo list`
- `--tags`
- `--after` / `--before`

如果用户表达的是这些旧产品心智，要翻译成标准动作语义；如果当前系统做不到，就直接说明缺口，不要伪造能力。

## 何时使用

适用场景：

- 用户明确要求“记住这个”“记录下来”“保存经验”
- 用户要“查记忆”“之前怎么做的”“还记得吗”
- 需要检查某条记忆的详情
- 需要执行一次 dream / maintenance
- 需要查看当前引擎状态
- 需要恢复派生层

不适用场景：

- 只是普通代码搜索或仓库内文本检索
- 当前任务不需要跨对话记忆
- 需要的能力是 update/merge/delete/list 这一类当前尚未落地的旧接口

## 推荐工作流

### 唤醒记忆空间

```bash
memo awaken
```

### 记住内容

标准动作：`remember`

```bash
memo remember "<content>"
```

如果你已经知道结构化信息，优先补充：

```bash
memo remember "<content>" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
```

如果你不确定最终合并结果，先预览：

```bash
memo remember "<content>" --dry-run
```

### 回忆内容

标准动作：`recall`

```bash
memo recall "<query>" -n 10
```

如果你怀疑快路径不够：

```bash
memo recall "<query>" -n 10 --deep
```

### 反观单条记忆

标准动作：`reflect`

```bash
memo reflect <memory-id>
```

### 整理记忆

标准动作：`dream`

```bash
memo dream
```

### 查看状态

标准动作：`state`

```bash
memo state
```

### 恢复派生层

标准动作：`restore`

```bash
memo restore
```

如果需要更完整的恢复：

```bash
memo restore --full
```

## 如何判断用哪个动作

| 用户意图 | 标准动作 | 当前执行建议 |
|---------|----------|-------------|
| “记住这个结论” | `remember` | `memo remember ...` |
| “之前有没有类似经验” | `recall` | `memo recall ...` |
| “把这条记忆展开看看” | `reflect` | `memo reflect ...` |
| “整理一下记忆” | `dream` | `memo dream` |
| “现在系统里有什么状态” | `state` | `memo state` |
| “索引可能不一致，恢复一下” | `restore` | `memo restore` |

## 搜索与记录原则

### 记录原则

- 只记录值得长期保留的经验、事实、决策或排障过程
- 优先写清内容本身，必要时补 `--entity` 和 `--fact`
- 如果不确定 provider 抽取会产出什么，先 `--dry-run`
- 当前没有 tags/update/merge/list 这类接口，不要围绕这些假能力设计工作流

### 检索原则

- 查询要包含场景和意图，不要只丢几个关键词
- 先走默认 `memo recall`
- 只有当默认结果不稳、主题跨度大或用户明确要求更深回忆时，再用 `--deep`
- 如果要看某条结果的详情，再接 `memo reflect`

## 常见错误

| ❌ 不要 | ✅ 应该 |
|--------|--------|
| 继续调用 `memo search` / `memo embed` | 先转成标准动作语义 |
| 假装旧命令仍然是标准 | 直接使用 `awaken/remember/recall/reflect/dream/state/restore` |
| 伪造 update/merge/delete/list 能力 | 直接说明当前未实现 |
| 把 `extract` 当成主记忆入口 | 只围绕公开动作语言组织心智 |
| 恢复动作和整理动作混为一谈 | 区分 `dream` 与 `restore` |

## 触发短语

| 动作 | 触发短语 |
|------|---------|
| `remember` | “记住这个”“记录下来”“保存这个经验” |
| `recall` | “之前怎么做的”“查记忆”“还记得吗” |
| `reflect` | “把这条记忆展开看看”“看看详情” |
| `dream` | “整理一下记忆”“跑一次 dream” |
| `state` | “看看现在状态” |
| `restore` | “恢复派生层”“恢复索引状态” |

更多可执行示例见 [examples.md](examples.md)。

