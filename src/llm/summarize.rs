use anyhow::Result;
use memo_types::{QueryResult, ScoreType};

use super::client::LlmClient;
use super::utils::escape_xml;

const SUMMARIZE_FRAMEWORK: &str = r#"你是一个知识整合专家。你的任务是根据用户的问题，整合相关记忆，生成一个综合回答。

整合策略：
<strategy>
{strategy}
</strategy>

用户问题：
<user_query>
{query}
</user_query>

相关记忆：
<memories>
{memories}
</memories>

请生成综合回答。"#;

const DEFAULT_SUMMARIZE_STRATEGY: &str = "根据问题类型调整输出风格：\n\
     - 事实查询（谁/什么/哪里）→ 直接回答\n\
     - 方法请求（如何/怎么做）→ 核心方法 + 关键细节\n\
     - 原因探究（为什么）→ 结论 + 原因分析\n\
     - 综合问题 → 逻辑结构（是什么→为什么→怎么做→案例→注意事项）\n\
     简洁优先，优先使用高分记忆，保持连贯自然表达。";

/// 使用 LLM 对搜索结果进行综合总结
///
/// `strategy` 为用户自定义总结策略段（只是策略描述），
/// 传 `None` 则使用内置默认策略。
pub async fn summarize_results(
    client: &LlmClient,
    original_query: &str,
    results: &[QueryResult],
    strategy: Option<&str>,
) -> Result<String> {
    if results.is_empty() {
        return Ok("没有找到相关记忆。".to_string());
    }

    let memories_text = build_memories_text(results);
    let strategy = strategy.unwrap_or(DEFAULT_SUMMARIZE_STRATEGY);
    let prompt = SUMMARIZE_FRAMEWORK
        .replace("{strategy}", strategy)
        .replace("{query}", &escape_xml(original_query))
        .replace("{memories}", &memories_text);

    client.chat(&prompt).await
}

fn build_memories_text(results: &[QueryResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let score_label = match r.score_type {
                Some(ScoreType::Rerank) => "排序分",
                _ => "相似度",
            };
            let score = r.score.unwrap_or(0.0);
            format!("[{}] ({}: {:.2})\n{}", i + 1, score_label, score, r.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}
