use anyhow::{Context, Result};
use quick_xml::de::from_str as xml_from_str;
use serde::Deserialize;

use super::client::LlmClient;
use super::utils::{escape_xml, extract_decomposition_xml, is_valid_dimension, is_valid_query};

/// 单个子问题
#[derive(Debug, Clone)]
pub struct SubQuery {
    pub dimension: String,
    pub query: String,
    pub needs_refinement: bool,
}

/// LLM 输出的 XML 结构（用于 serde 反序列化）
#[derive(Debug, Deserialize)]
struct DecompositionXml {
    #[serde(rename = "subquery")]
    subqueries: Vec<SubQueryXml>,
}

#[derive(Debug, Deserialize)]
struct SubQueryXml {
    dimension: String,
    query: String,
    needs_refinement: String,
}

/// 从 query 拆解出子问题列表
pub async fn decompose_query(client: &LlmClient, query: &str) -> Result<Vec<SubQuery>> {
    let escaped = escape_xml(query);
    let prompt = build_decompose_prompt(&escaped);
    let output = client.chat(&prompt).await?;

    tracing::debug!("LLM decompose output: {}", output);

    let xml = extract_decomposition_xml(&output)?;
    parse_decomposition_xml(&xml)
}

fn build_decompose_prompt(escaped_query: &str) -> String {
    format!(
        r#"你是一个搜索问题分析专家。请将给定的问题按五维模型拆解为子问题。

五维拆解模型：
- core（核）：核心问题
- why（因）：原因/原理
- how（法）：方法/步骤
- case（例）：案例/实践
- note（注）：注意事项

拆解规则：
1. 选择 3-5 个最相关的维度（不必全部使用）
2. 每个维度生成一个子问题
3. 子问题应该是完整的自然语言问句
4. 判断每个子问题是否需要进一步拆解（needs_refinement: true/false）
   - false: 问题足够具体，可以直接搜索
   - true: 问题太宽泛，需要继续拆解

输出格式（XML）：

<decomposition>
  <subquery>
    <dimension>core</dimension>
    <query>完整的问句</query>
    <needs_refinement>false</needs_refinement>
  </subquery>
</decomposition>

当前问题：
<user_query>
{query}
</user_query>

输出要求：
1. 只输出 XML 格式，不要其他文字
2. 从 <decomposition> 开始到 </decomposition> 结束
3. 不要添加代码块标记"#,
        query = escaped_query
    )
}

fn parse_decomposition_xml(xml: &str) -> Result<Vec<SubQuery>> {
    let parsed: DecompositionXml =
        xml_from_str(xml).context("Failed to parse LLM XML output")?;

    let subqueries: Vec<SubQuery> = parsed
        .subqueries
        .into_iter()
        .filter_map(|sq| {
            if is_valid_dimension(&sq.dimension) && is_valid_query(&sq.query) {
                Some(SubQuery {
                    needs_refinement: sq.needs_refinement.trim() == "true",
                    dimension: sq.dimension,
                    query: sq.query,
                })
            } else {
                tracing::warn!(
                    "Skipping invalid subquery: dim={}, query_len={}",
                    sq.dimension,
                    sq.query.len()
                );
                None
            }
        })
        .collect();

    if subqueries.is_empty() {
        anyhow::bail!("No valid subqueries parsed from LLM output");
    }

    Ok(subqueries)
}
