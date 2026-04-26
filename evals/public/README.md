# Public Benchmark Adapters

This directory is reserved for normalized public benchmark inputs and adapter notes.

The engine-level evaluation runner accepts a normalized JSONL event stream before any concrete public dataset parser is added:

```jsonl
{"type":"memory","id":"m1","content":"Alice confirmed London is her current home."}
{"type":"query","id":"q1","aspect":"public_temporal","query":"Where does Alice live?","expected_memory_ids":["m1"],"deep":true}
{"type":"query","id":"q2","aspect":"public_abstention","query":"Where does Bob live?","should_abstain":true}
```

Concrete LongMemEval, LOCOMO, or other public benchmark parsers must verify the current upstream dataset format before implementation. They should convert source-specific fields into the normalized event stream instead of changing `EvalDataset`.
