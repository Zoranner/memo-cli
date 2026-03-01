use anyhow::{Context, Result};

/// 转义 XML 特殊字符，防止 prompt injection
pub fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 从 LLM 输出中提取 <decomposition>...</decomposition> 块
pub fn extract_decomposition_xml(output: &str) -> Result<String> {
    let start = output
        .find("<decomposition>")
        .context("LLM output missing <decomposition> tag")?;
    let end = output
        .find("</decomposition>")
        .context("LLM output missing </decomposition> tag")?;
    let end = end + "</decomposition>".len();
    Ok(output[start..end].to_string())
}

const VALID_DIMENSIONS: &[&str] = &["core", "why", "how", "case", "note"];

const MIN_QUERY_LEN: usize = 5;
const MAX_QUERY_LEN: usize = 300;

/// 验证维度是否有效
pub fn is_valid_dimension(dim: &str) -> bool {
    VALID_DIMENSIONS.contains(&dim)
}

/// 验证子问题内容长度
pub fn is_valid_query(query: &str) -> bool {
    let len = query.trim().len();
    len >= MIN_QUERY_LEN && len <= MAX_QUERY_LEN
}
