# Memory Examples

These examples use the public action language directly.

## Awaken a Memory Space

```bash
memo awaken
```

## remember: Write Structured Memory

```bash
memo remember "Alice lives in Paris" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
```

Use this when:

- You already know the important entities and facts
- You want later recall to hit structured results more reliably

## remember: Preview First with Dry Run

```bash
memo remember "Alice lives in Paris and often signs as Ally." --entity person:Alice:Ally --dry-run
```

After checking the preview, write it for real:

```bash
memo remember "Alice lives in Paris and often signs as Ally." --entity person:Alice:Ally
```

Use this when:

- You are unsure how provider extraction and manual input will merge
- You want to inspect entities/facts before writing

## recall: Default Retrieval

```bash
memo recall "Where does Alice live?" -n 5
```

Use this when:

- You want ordinary memory retrieval
- You want the system to try the default fast path first

## recall: Force Deep Search

```bash
memo recall "Alice travel history and city relationships" -n 10 --deep
```

Use this when:

- The topic spans multiple layers
- Default recall results look weak
- You explicitly need deeper recall

## reflect: Inspect One Memory Record

```bash
memo reflect <memory-id>
```

Use this when:

- You want the full content of one recall result
- You want to inspect layer, reasons, or related detail

## dream: Consolidate Memory

```bash
memo dream
```

Use this when:

- You just wrote a batch of memories and want a manual consolidation pass
- You want to advance promotion, cooling, archival, and conflict resolution

## state: Inspect Current State

```bash
memo state
```

Use this when:

- You want episode/entity/fact/edge counts
- You need to confirm whether text/vector indexes are pending
- You want dream queue status

## restore: Restore Pending Derived Layers

```bash
memo restore
```

Use this when:

- Remember has left indexes in pending state
- You want the conservative restore path

## restore: Full Rebuild of Derived Indexes

```bash
memo restore --full
```

Use this when:

- Derived indexes may be damaged or stale
- You explicitly want a full restore from SQLite truth source

## Old Interfaces That Are Not Supported Now

Do not keep using these old commands or parameters:

- `memo embed`
- `memo search`
- `memo update`
- `memo merge`
- `memo delete`
- `memo list`
- `--tags`
- `--after` / `--before`



