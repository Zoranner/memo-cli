# 记忆示例集

这些示例直接使用公开动作语言。

## 唤醒记忆空间

```bash
memo awaken
```

## remember：写入一条结构化记忆

```bash
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
```

适用场景：

- 你已经知道核心实体和事实
- 希望后续 recall 时更容易命中结构化结果

## remember：写入带别名的记忆

```bash
memo remember "Alice lives in Paris and often signs as Ally." --entity person:Alice:Ally
```

适用场景：

- 你已经知道实体别名
- 希望后续用别名 recall 时也能命中

## recall：默认回忆

```bash
memo recall "Where does Alice live?" -n 5
```

适用场景：

- 正常查找历史记忆
- 先让系统走默认快路径

## recall：强制 deep search

```bash
memo recall "Alice travel history and city relationships" -n 10 --deep
```

适用场景：

- 主题跨度较大
- 默认 recall 结果不稳
- 你明确需要更深一层的回忆

## reflect：查看单条记忆详情

```bash
memo reflect <memory-id>
```

适用场景：

- 想确认某条 recall 结果的完整内容
- 想看 layer、reasons 或关联细节

## dream：整理和巩固记忆

```bash
memo dream
```

适用场景：

- 刚写入了一批记忆，想手动整理一次
- 想推进记忆晋升、冷却、归档和冲突收敛

## state：查看当前状态

```bash
memo state
```

适用场景：

- 想看 episode/entity/fact/edge 数量
- 想确认 text/vector 索引是否 pending
- 想看 provider readiness 和是否需要运行 dream

## dream：维护派生层

```bash
memo dream
```

适用场景：

- remember 之后索引还处于 pending
- 想让 text/vector 派生层追上 SQLite 真相源

## dream：完整重建派生层

```bash
memo dream --full
```

适用场景：

- 派生索引可能损坏
- 你明确要基于 SQLite 真相源做完整派生层维护

## 当前不支持的旧接口

下面这些旧命令和参数，不要继续使用：

- `memo embed`
- `memo search`
- `memo restore`
- `memo update`
- `memo merge`
- `memo delete`
- `memo list`
- `--tags`
- `--after` / `--before`


