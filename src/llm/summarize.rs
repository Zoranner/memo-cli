use anyhow::Result;
use memo_types::QueryResult;

use super::client::LlmClient;
use super::utils::escape_xml;

/// 使用 LLM 对搜索结果进行综合总结
pub async fn summarize_results(
    client: &LlmClient,
    original_query: &str,
    results: &[QueryResult],
) -> Result<String> {
    if results.is_empty() {
        return Ok("没有找到相关记忆。".to_string());
    }

    let memories_text = build_memories_text(results);
    let escaped_query = escape_xml(original_query);
    let prompt = build_summarize_prompt(&escaped_query, &memories_text);

    client.chat(&prompt).await
}

fn build_memories_text(results: &[QueryResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let score = r.score.unwrap_or(0.0);
            format!("[{}] (相似度: {:.2})\n{}", i + 1, score, r.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn build_summarize_prompt(escaped_query: &str, memories_text: &str) -> String {
    format!(
        r#"你是一个知识整合专家。根据用户问题，用最合适的方式整合相关记忆。

用户问题：
<user_query>
{query}
</user_query>

相关记忆：
<memories>
{memories}
</memories>

整合要求：
1. 根据问题类型调整输出风格
   - 事实查询（谁/什么/哪里）→ 直接回答
   - 方法请求（如何/怎么做）→ 核心方法 + 关键细节
   - 原因探究（为什么）→ 结论 + 原因分析
   - 综合问题 → 逻辑结构（是什么→为什么→怎么做→案例→注意事项）

2. 简洁优先：简单问题不要画蛇添足

3. 相似度优先：优先使用高分记忆

4. 自然表达：保持连贯，避免机械堆砌

请整合生成答案。"#,
        query = escaped_query,
        memories = memories_text
    )
}
