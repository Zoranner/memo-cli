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

/// 从 LLM 输出中提取 <queries>...</queries> 块
pub fn extract_queries_xml(output: &str) -> Result<String> {
    let start = output
        .find("<queries>")
        .context("LLM output missing <queries> tag")?;
    let end = output
        .find("</queries>")
        .context("LLM output missing </queries> tag")?;
    let end = end + "</queries>".len();
    Ok(output[start..end].to_string())
}

const MIN_QUERY_LEN: usize = 5;
const MAX_QUERY_LEN: usize = 300;

/// 验证子问题内容长度
pub fn is_valid_query(query: &str) -> bool {
    let len = query.trim().len();
    (MIN_QUERY_LEN..=MAX_QUERY_LEN).contains(&len)
}
