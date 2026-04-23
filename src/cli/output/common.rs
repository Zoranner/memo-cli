use anyhow::Result;
use serde::Serialize;

pub(crate) fn render_json_or_text<T: Serialize>(
    value: &T,
    human: &str,
    json: bool,
) -> Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(value)?)
    } else {
        Ok(human.to_string())
    }
}

pub(crate) fn created_label(created: bool) -> &'static str {
    if created {
        "created"
    } else {
        "kept"
    }
}
