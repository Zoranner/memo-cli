---
name: memo-brain
description: Manage and retrieve cross-conversation memory. Public action semantics follow command-philosophy. Use for "remember this", "search memory", "show current state", or restore flows.
---

# Memo Brain Management

This skill follows the public action language defined in `docs/architecture/command-philosophy.md`.

## Standard Actions

- `memo awaken`
- `memo remember`
- `memo recall`
- `memo reflect`
- `memo dream`
- `memo state`
- `memo restore`

## Current Capability Boundaries

The current CLI does **not** provide these old interfaces. Do not reason or act as if they still exist:

- `memo embed`
- `memo search`
- `memo update`
- `memo merge`
- `memo delete`
- `memo list`
- `--tags`
- `--after` / `--before`

If the user speaks in that old product language, translate it into the standard action semantics. If the current system cannot support the request, say so directly instead of fabricating capabilities.

## When to Use

Use this skill when:

- The user explicitly asks to remember or record something
- The user wants to search or recall past memory
- You need details for one memory record
- You need to run dream / maintenance
- You need current engine state
- You need to restore derived layers

Do not use this skill when:

- The task is ordinary code search inside the repository
- The task does not need cross-conversation memory
- The request depends on update/merge/delete/list behaviors that do not exist in the current CLI

## Recommended Workflow

### Awaken a Memory Space

```bash
memo awaken
```

### Remember Content

Standard action: `remember`

```bash
memo remember "<content>"
```

If you already know structured information, add it explicitly:

```bash
memo remember "<content>" --entity person:Alice --entity place:Paris --fact Alice:lives_in:Paris
```

If you are unsure what the final merged payload will look like, preview first:

```bash
memo remember "<content>" --dry-run
```

### Recall Content

Standard action: `recall`

```bash
memo recall "<query>" -n 10
```

If fast-path retrieval is likely insufficient:

```bash
memo recall "<query>" -n 10 --deep
```

### Reflect on One Memory

Standard action: `reflect`

```bash
memo reflect <memory-id>
```

### Dream

Standard action: `dream`

```bash
memo dream
```

### Inspect State

Standard action: `state`

```bash
memo state
```

### Restore Derived Layers

Standard action: `restore`

```bash
memo restore
```

For explicit full restore:

```bash
memo restore --full
```

## How to Choose the Action

| User Intent | Standard Action | Current Execution |
|-------------|-----------------|-------------------|
| "remember this conclusion" | `remember` | `memo remember ...` |
| "did we solve something like this before" | `recall` | `memo recall ...` |
| "show me that memory in detail" | `reflect` | `memo reflect ...` |
| "organize the memory" | `dream` | `memo dream` |
| "what is the current system state" | `state` | `memo state` |
| "indexes may be inconsistent, restore them" | `restore` | `memo restore` |

## Retrieval and Recording Principles

### Recording Principles

- Record durable experience, facts, decisions, or troubleshooting outcomes worth keeping
- Focus on the content first; add `--entity` and `--fact` when you can do so concretely
- If provider extraction may change the final payload, use `--dry-run` first
- Do not design workflows around nonexistent features such as tags, update, merge, or list

### Retrieval Principles

- Queries should include situation and intent, not only loose keywords
- Start with default `memo recall`
- Only use `--deep` when default results look weak, the topic spans multiple layers, or the user explicitly asks for deeper recall
- Use `memo reflect` when you need detail on one returned record

## Common Mistakes

| Don't | Do |
|-------|----|
| Keep calling `memo search` / `memo embed` | Translate into standard action semantics |
| Pretend old commands are still the standard | Use `awaken/remember/recall/reflect/dream/state/restore` directly |
| Fake update/merge/delete/list capabilities | State directly that they are not implemented in the current CLI |
| Treat `extract` as the main memory entrypoint | Organize the workflow around public actions only |
| Mix up restore and dream | Keep `restore` and `dream` as different actions |

## Trigger Phrases

| Action | Trigger Phrases |
|--------|-----------------|
| `remember` | "remember this", "record this", "save this experience" |
| `recall` | "how did we do it before", "search memory", "do you remember" |
| `reflect` | "show that memory in detail", "open the details" |
| `dream` | "organize memory", "run dream" |
| `state` | "show current state" |
| `restore` | "restore derived layer", "restore index state" |

For executable examples, see [examples.md](examples.md).

