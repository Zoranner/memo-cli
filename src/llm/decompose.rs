use anyhow::{Context, Result};
use quick_xml::de::from_str as xml_from_str;
use serde::Deserialize;

use super::client::LlmClient;
use super::utils::{escape_xml, extract_queries_xml, is_valid_query};

const DECOMPOSE_FRAMEWORK: &str = r#"你是一个搜索问题分析专家。你的任务是将用户的搜索问题拆解为多个可以独立搜索的子问题。

拆解策略：
<strategy>
{strategy}
</strategy>

当前问题：
<user_query>
{query}
</user_query>

请按以下 XML 格式输出子问题，从 <queries> 开始到 </queries> 结束，不要输出其他内容：

<queries>
  <query>
    <question>子问题（完整问句）</question>
    <need_expand>false</need_expand>
  </query>
</queries>

字段说明：
- question：完整的问句，应该是可以独立搜索的
- need_expand：若该子问题仍然宽泛需要进一步拆解则填 true，否则填 false"#;

const DEFAULT_DECOMPOSE_STRATEGY: &str = "请按以下五维模型拆解问题，选择 3-5 个最相关的维度：\n\
     - core（核心）：问题的核心是什么\n\
     - why（原因）：为什么会有这个问题\n\
     - how（方法）：如何解决这个问题\n\
     - case（案例）：有哪些实际案例\n\
     - note（注意）：需要注意什么";

/// 单个子问题
#[derive(Debug, Clone)]
pub struct SubQuery {
    pub question: String,
    pub need_expand: bool,
}

/// LLM 输出的 XML 结构（用于 serde 反序列化）
#[derive(Debug, Deserialize)]
struct QueriesXml {
    query: Vec<SubQueryXml>,
}

#[derive(Debug, Deserialize)]
struct SubQueryXml {
    question: String,
    #[serde(default)]
    need_expand: String, // "true" / "false"，默认 "false"
}

/// 从 query 拆解出子问题列表
///
/// `strategy` 为用户自定义策略段（只是策略描述，不含 XML 格式要求），
/// 传 `None` 则使用内置五维拆解策略。
pub async fn decompose_query(
    client: &LlmClient,
    query: &str,
    strategy: Option<&str>,
) -> Result<Vec<SubQuery>> {
    let strategy = strategy.unwrap_or(DEFAULT_DECOMPOSE_STRATEGY);
    let prompt = DECOMPOSE_FRAMEWORK
        .replace("{strategy}", strategy)
        .replace("{query}", &escape_xml(query));

    let output = client.chat(&prompt).await?;
    tracing::debug!("LLM decompose output: {}", output);

    let xml = extract_queries_xml(&output)?;
    parse_queries_xml(&xml)
}

fn parse_queries_xml(xml: &str) -> Result<Vec<SubQuery>> {
    let parsed: QueriesXml = xml_from_str(xml).context("Failed to parse LLM XML output")?;

    let subqueries: Vec<SubQuery> = parsed
        .query
        .into_iter()
        .filter_map(|sq| {
            if is_valid_query(&sq.question) {
                Some(SubQuery {
                    need_expand: sq.need_expand.trim() == "true",
                    question: sq.question,
                })
            } else {
                tracing::warn!(
                    "Skipping invalid subquery: question_len={}",
                    sq.question.len()
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
