mod adapter;
mod normalize;
mod prompt;

pub(crate) use adapter::LmkitExtractionAdapter;
pub(crate) use normalize::{parse_extraction_response_with_options, ExtractionCleanupOptions};
pub(crate) use prompt::EXTRACTION_SYSTEM_PROMPT;

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{parse_extraction_response_with_options, ExtractionCleanupOptions};

    #[test]
    fn parse_extraction_response_reads_entities_and_facts() -> Result<()> {
        let result = parse_extraction_response_with_options(
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
            ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].predicate, "lives_in");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_accepts_json_code_fence() -> Result<()> {
        let result = parse_extraction_response_with_options(
            "```json\n{\"entities\":[],\"facts\":[]}\n```",
            ExtractionCleanupOptions::default(),
        )?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }

    #[test]
    fn parse_extraction_response_normalizes_predicate_format() -> Result<()> {
        let result = parse_extraction_response_with_options(
            r#"{
                "entities": [],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "Lives In",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.facts[0].predicate, "lives_in");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_filters_low_confidence_and_empty_fields() -> Result<()> {
        let result = parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "  ",
                        "aliases": ["A"],
                        "confidence": 0.9
                    },
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": [" Ally ", ""],
                        "confidence": 0.2
                    }
                ],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "lives_in",
                        "object": "Paris",
                        "confidence": 0.2
                    },
                    {
                        "subject": "Alice",
                        "predicate": "  ",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            ExtractionCleanupOptions::default(),
        )?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }

    #[test]
    fn parse_extraction_response_extracts_json_object_from_wrapped_text() -> Result<()> {
        let result = parse_extraction_response_with_options(
            r#"Here is the extracted memory:

            {
              "entities": [
                {
                  "entity_type": "person",
                  "name": "Alice",
                  "aliases": ["Ally"],
                  "confidence": 0.92
                }
              ],
              "facts": []
            }

            Hope this helps."#,
            ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_merges_duplicate_entities_and_facts() -> Result<()> {
        let result = parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "alice",
                        "aliases": ["ally", "A."],
                        "confidence": 0.6
                    },
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": ["Ally"],
                        "confidence": 0.9
                    }
                ],
                "facts": [
                    {
                        "subject": "Ally",
                        "predicate": "Lives In",
                        "object": " Paris ",
                        "confidence": 0.7
                    },
                    {
                        "subject": "alice",
                        "predicate": "lives_in",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        assert_eq!(result.entities[0].confidence, 0.9);
        assert!(result.entities[0].aliases.contains(&"Ally".to_string()));
        assert!(result.entities[0].aliases.contains(&"A.".to_string()));

        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].subject, "Alice");
        assert_eq!(result.facts[0].object, "Paris");
        assert_eq!(result.facts[0].predicate, "lives_in");
        assert_eq!(result.facts[0].confidence, 0.9);
        Ok(())
    }

    #[test]
    fn parse_extraction_response_respects_min_confidence_option() -> Result<()> {
        let result = parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": [],
                        "confidence": 0.65
                    }
                ],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "Lives In",
                        "object": "Paris",
                        "confidence": 0.65
                    }
                ]
            }"#,
            ExtractionCleanupOptions {
                min_confidence: 0.7,
                normalize_predicates: true,
            },
        )?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }

    #[test]
    fn parse_extraction_response_can_disable_predicate_normalization() -> Result<()> {
        let result = parse_extraction_response_with_options(
            r#"{
                "entities": [],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "Lives In",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            ExtractionCleanupOptions {
                min_confidence: 0.5,
                normalize_predicates: false,
            },
        )?;

        assert_eq!(result.facts[0].predicate, "Lives In");
        Ok(())
    }
}
