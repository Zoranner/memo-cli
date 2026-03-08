use anyhow::{Context, Result};
use serde::Deserialize;

use super::client::LlmClient;
use super::utils::escape_xml;

const DECOMPOSE_PROMPT: &str = r#"你是一个搜索问题分析专家。将用户的搜索问题拆解为子问题树，用于多路并行检索。

拆解建议：
- 树的深度建议 2-3 层，通常无需超过 4 层
- 每个节点子问题建议 3-5 个
- 子问题足够具体可直接检索时，children 必须为空数组 []

拆解策略：
<strategy>
{strategy}
</strategy>

当前问题：
<user_query>
{query}
</user_query>

严格按以下 JSON 格式输出，不要包含任何其他内容或解释：
```json
[
  {
    "question": "子问题（完整问句）",
    "children": [
      {"question": "孙问题（足够具体无需展开）", "children": []}
    ]
  }
]
```"#;

const DEFAULT_STRATEGY: &str = "按以下五维模型选择 3-5 个最相关维度拆解：\n\
     - core（核心）：问题的核心是什么\n\
     - why（原因）：为什么会有这个问题\n\
     - how（方法）：如何解决这个问题\n\
     - case（案例）：有哪些实际案例\n\
     - note（注意）：需要注意什么";

const REASK_SUFFIX: &str =
    "\n\n[上次输出无法解析为 JSON，错误：{error}。请只输出 JSON 数组，不要包含代码块标记（如 ```json）或其他文字。]";

/// 子问题树节点
#[derive(Debug, Clone)]
pub struct SubQueryTree {
    pub question: String,
    pub children: Vec<SubQueryTree>,
}

/// 超过此深度时强制作为叶子，防止 LLM 异常输出导致递归失控
const MAX_SAFE_DEPTH: usize = 8;

impl SubQueryTree {
    pub fn leaves(&self) -> Vec<String> {
        let mut result = Vec::new();
        self.collect_leaves(0, &mut result);
        result
    }

    fn collect_leaves(&self, depth: usize, result: &mut Vec<String>) {
        if self.children.is_empty() || depth >= MAX_SAFE_DEPTH {
            let q = self.question.trim();
            if q.len() >= 5 {
                result.push(q.to_string());
            } else {
                tracing::debug!("Skipping leaf with too short question: {}", q.len());
            }
        } else {
            for child in &self.children {
                child.collect_leaves(depth + 1, result);
            }
        }
    }
}

/// JSON 反序列化用的内部类型（递归）
#[derive(Debug, Deserialize)]
struct SubQueryJson {
    question: String,
    #[serde(default)]
    children: Vec<SubQueryJson>,
}

/// 将用户查询拆解为子问题树，单次 LLM 调用，失败时 Reask 一次
pub async fn decompose_query_tree(
    client: &LlmClient,
    query: &str,
    strategy: Option<&str>,
) -> Result<Vec<SubQueryTree>> {
    let strategy = strategy.unwrap_or(DEFAULT_STRATEGY);
    let prompt = build_prompt(query, strategy);

    let output = client.chat(&prompt).await?;
    match parse_tree_json(&output) {
        Ok(trees) => Ok(trees),
        Err(e) => {
            tracing::debug!("First attempt JSON parse failed: {}", e);
            let reask_prompt = format!(
                "{}{}",
                prompt,
                REASK_SUFFIX.replace("{error}", &e.to_string())
            );
            let output = client.chat(&reask_prompt).await?;
            parse_tree_json(&output).context("Reask also failed to produce valid JSON")
        }
    }
}

fn build_prompt(query: &str, strategy: &str) -> String {
    DECOMPOSE_PROMPT
        .replace("{strategy}", strategy)
        .replace("{query}", &escape_xml(query))
}

/// 多策略提取 + 即时验证：每种策略提取完立刻尝试解析，成功则返回，全部失败才报错
fn parse_tree_json(output: &str) -> Result<Vec<SubQueryTree>> {
    // 策略 1：从代码块内提取
    if let Some(json_str) = extract_from_code_fence(output) {
        if let Ok(nodes) = try_parse_nodes(&json_str) {
            return Ok(nodes);
        }
    }
    // 策略 2：方括号边界匹配
    if let Some(json_str) = extract_by_brackets(output) {
        if let Ok(nodes) = try_parse_nodes(&json_str) {
            return Ok(nodes);
        }
    }
    // 策略 3：整段直接解析（LLM 直接输出纯 JSON 的情况）
    if let Ok(nodes) = try_parse_nodes(output.trim()) {
        return Ok(nodes);
    }

    anyhow::bail!("Failed to extract valid JSON array from LLM output")
}

fn try_parse_nodes(json_str: &str) -> Result<Vec<SubQueryTree>> {
    let nodes: Vec<SubQueryJson> = serde_json::from_str(json_str)?;
    if nodes.is_empty() {
        anyhow::bail!("Empty query tree");
    }
    Ok(nodes.into_iter().map(convert_node).collect())
}

fn extract_from_code_fence(output: &str) -> Option<String> {
    let fence_start = output.find("```")?;
    let after_fence = &output[fence_start + 3..];

    let after_lang = if after_fence.starts_with("json") {
        after_fence[4..].trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ')
    } else {
        after_fence.trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ')
    };

    let close = after_lang.find("```")?;
    let inner = after_lang[..close].trim();

    if inner.starts_with('[') {
        Some(inner.to_string())
    } else {
        None
    }
}

fn extract_by_brackets(output: &str) -> Option<String> {
    let start = output.find('[')?;
    let end = output.rfind(']')?;
    if end < start {
        return None;
    }
    Some(output[start..=end].to_string())
}

fn convert_node(json: SubQueryJson) -> SubQueryTree {
    SubQueryTree {
        question: json.question,
        children: json.children.into_iter().map(convert_node).collect(),
    }
}
