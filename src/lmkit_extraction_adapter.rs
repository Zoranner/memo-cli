use anyhow::{Context, Result};
use lmkit::{
    create_chat_provider, ChatMessage, ChatProvider as LmkitChatProvider, ChatRequest,
    ProviderConfig, RequestPreset, ResponseFormat,
};
use memo_engine::{ExtractionProvider, ExtractionResult};
use serde::Deserialize;
use tokio::runtime::{Builder, Runtime};

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You extract memory facts from user text.
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

pub(crate) struct LmkitExtractionAdapter {
    runtime: Runtime,
    provider: Box<dyn LmkitChatProvider>,
}

impl LmkitExtractionAdapter {
    pub(crate) fn new(config: ProviderConfig) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime for lmkit extraction")?;
        let provider =
            create_chat_provider(&config).context("failed to create lmkit chat provider")?;

        Ok(Self { runtime, provider })
    }
}

impl ExtractionProvider for LmkitExtractionAdapter {
    fn extract(&self, text: &str) -> Result<ExtractionResult> {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system(EXTRACTION_SYSTEM_PROMPT),
                ChatMessage::user(text),
            ],
            response_format: Some(ResponseFormat::JsonObject),
            preset: Some(RequestPreset::Execution),
            temperature: Some(0.0),
            ..Default::default()
        };
        let response = self
            .runtime
            .block_on(self.provider.complete(&request))
            .context("lmkit extraction request failed")?;
        let content = response
            .content
            .as_deref()
            .with_context(|| "lmkit extraction response missing content".to_string())?;

        parse_extraction_response(content)
    }
}

#[derive(Deserialize)]
struct ExtractionEnvelope {
    #[serde(default)]
    result: Option<ExtractionResult>,
    #[serde(default)]
    entities: Vec<memo_engine::ExtractedEntity>,
    #[serde(default)]
    facts: Vec<memo_engine::ExtractedFact>,
}

pub(crate) fn parse_extraction_response(content: &str) -> Result<ExtractionResult> {
    let json = strip_json_fence(content);
    if let Ok(result) = serde_json::from_str::<ExtractionResult>(json) {
        return Ok(normalize_extraction_result(result));
    }

    let envelope: ExtractionEnvelope =
        serde_json::from_str(json).context("failed to parse extraction JSON response")?;
    let result = envelope.result.unwrap_or(ExtractionResult {
        entities: envelope.entities,
        facts: envelope.facts,
    });
    Ok(normalize_extraction_result(result))
}

fn strip_json_fence(content: &str) -> &str {
    let trimmed = content.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    trimmed
}

fn normalize_extraction_result(mut result: ExtractionResult) -> ExtractionResult {
    result.entities = result
        .entities
        .into_iter()
        .filter_map(|mut entity| {
            entity.name = entity.name.trim().to_string();
            entity.entity_type = entity.entity_type.trim().to_string();
            entity.aliases = entity
                .aliases
                .into_iter()
                .map(|alias| alias.trim().to_string())
                .filter(|alias| !alias.is_empty())
                .collect();
            entity.confidence = entity.confidence.clamp(0.0, 1.0);
            if entity.name.is_empty() || entity.entity_type.is_empty() {
                None
            } else {
                Some(entity)
            }
        })
        .collect();
    result.facts = result
        .facts
        .into_iter()
        .filter_map(|mut fact| {
            fact.subject = fact.subject.trim().to_string();
            fact.predicate = fact.predicate.trim().to_string();
            fact.object = fact.object.trim().to_string();
            fact.confidence = fact.confidence.clamp(0.0, 1.0);
            if fact.subject.is_empty() || fact.predicate.is_empty() || fact.object.is_empty() {
                None
            } else {
                Some(fact)
            }
        })
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[test]
    fn parse_extraction_response_reads_entities_and_facts() -> Result<()> {
        let result = super::parse_extraction_response(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": ["Ally"],
                        "confidence": 0.91
                    }
                ],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "lives_in",
                        "object": "Paris",
                        "confidence": 0.88
                    }
                ]
            }"#,
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].predicate, "lives_in");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_accepts_json_code_fence() -> Result<()> {
        let result =
            super::parse_extraction_response("```json\n{\"entities\":[],\"facts\":[]}\n```")?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }
}
