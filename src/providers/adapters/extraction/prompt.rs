pub(crate) const EXTRACTION_SYSTEM_PROMPT: &str = r#"You extract memory facts from user text.
Return strict JSON only.

Schema:
{
  "entities": [
    {
      "entity_type": "person|place|organization|project|pet|unknown",
      "name": "canonical entity name",
      "aliases": ["alias"],
      "confidence": 0.0
    }
  ],
  "facts": [
    {
      "subject": "entity name",
      "predicate": "snake_case_relation",
      "object": "entity or value text",
      "confidence": 0.0
    }
  ]
}

Rules:
- Output valid JSON object only, no prose.
- If nothing reliable is found, return {"entities":[],"facts":[]}.
- Keep confidence in [0,1].
- Use concise canonical names.
- Facts must be directly supported by the input text.
"#;
