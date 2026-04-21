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

## remember：先 dry-run 再落库

```bash
memo remember "Alice lives in Paris and often signs as Ally." --entity person:Alice:Ally --dry-run
```

看完预览后再正式写入：

```bash
memo remember "Alice lives in Paris and often signs as Ally." --entity person:Alice:Ally
```

适用场景：

- 你不确定 provider 抽取和手工输入合并后的结果
- 你想先检查 entities/facts 是否合理

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
- 想看 dream 队列状态

## restore：刷新 pending 派生索引

```bash
memo restore
```

适用场景：

- remember 之后索引还处于 pending
- 想走较保守的 restore 路径

## restore：全量重建派生索引

```bash
memo restore --full
```

适用场景：

- 派生索引可能损坏
- 你明确要基于 SQLite 真相源做全量恢复

## 当前不支持的旧接口

下面这些旧命令和参数，不要继续使用：

- `memo embed`
- `memo search`
- `memo update`
- `memo merge`
- `memo delete`
- `memo list`
- `--tags`
- `--after` / `--before`


