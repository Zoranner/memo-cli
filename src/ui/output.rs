use console::Style;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use memo_types::{QueryResult, ScoreType};

/// 已在业务层通过 [`Output::error`] / [`Output::error_report`] 向用户展示过的错误；
/// 回到 `main` 时不应再次打印（但仍以非零退出码结束进程）。
#[derive(Debug, Clone, Copy, Default)]
pub struct AlreadyReported;

impl std::fmt::Display for AlreadyReported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("error was already reported")
    }
}

impl std::error::Error for AlreadyReported {}

/// `main` 在打印顶层错误前调用：已由 [`Output::fail`] 等方式标记过的错误不再重复打印。
pub fn is_already_reported_root(err: &anyhow::Error) -> bool {
    err.downcast_ref::<AlreadyReported>().is_some()
}

/// 命令行输出格式化工具
/// 提供统一的 Cargo 风格输出
pub struct Output {
    green: Style,
    bold: Style,
    dim: Style,
    silent: bool,
}

impl Output {
    // === 构造方法 ===

    pub fn new() -> Self {
        Self {
            green: Style::new().green().bold(),
            bold: Style::new().bold(),
            dim: Style::new().dim(),
            silent: false,
        }
    }

    // === 状态和进度显示方法 ===

    /// 显示状态消息（如 "Loading model", "Embedding text" 等）
    /// 格式: "     Loading model ..."（动词右对齐到 12 字符）
    pub fn status(&self, action: &str, target: &str) {
        if self.silent {
            return;
        }
        eprintln!("{:>12} {}", self.green.apply_to(action), target);
    }

    /// 带耗时的状态消息，格式: "  Decomposed 12 sub-questions [3.2s]"
    pub fn status_timed(&self, action: &str, target: &str, elapsed: Duration) {
        if self.silent {
            return;
        }
        eprintln!(
            "{:>12} {} {}",
            self.green.apply_to(action),
            target,
            self.dim
                .apply_to(format!("[{:.1}s]", elapsed.as_secs_f32()))
        );
    }

    /// 显示子查询树（紧跟 Decomposed 状态行之后）
    pub fn sub_query_tree(&self, lines: &[String]) {
        if self.silent {
            return;
        }
        for line in lines {
            eprintln!("{:>12} {}", "", line);
        }
    }

    /// 开始执行操作的状态消息（会在前面自动添加空行）
    /// 用于标记一个新操作的开始，例如用户确认后的实际执行
    pub fn begin_operation(&self, action: &str, target: &str) {
        if self.silent {
            return;
        }
        eprintln!();
        eprintln!("{:>12} {}", self.green.apply_to(action), target);
    }

    /// 显示完成消息
    /// 格式: "    Finished action for scope"
    /// 自动在前面添加空行
    pub fn finish(&self, action: &str, scope: &str) {
        if self.silent {
            return;
        }
        eprintln!();
        eprintln!(
            "{:>12} {} for {} scope",
            self.green.apply_to("Finished"),
            action,
            scope
        );
    }

    /// 显示总耗时，格式: "    Finished in 6.2s"
    pub fn finished(&self, elapsed: Duration) {
        if self.silent {
            return;
        }
        eprintln!();
        eprintln!(
            "{:>12} in {:.1}s",
            self.green.apply_to("Finished"),
            elapsed.as_secs_f32()
        );
    }

    // === 信息显示方法 ===

    /// 显示数据库信息
    /// 格式: "    Database /path/to/db (123 records)"
    /// 自动在后面添加空行
    pub fn database_info(&self, path: &Path, record_count: usize) {
        if self.silent {
            return;
        }
        eprintln!(
            "{:>12} {} {}",
            self.green.apply_to("Database"),
            path.display(),
            self.dim.apply_to(format!("({} records)", record_count))
        );
        eprintln!();
    }

    /// 显示数据库信息（带模型）
    /// 格式: "    Database /path/to/db (123 records, text-embedding-v4/1024d)"
    /// 自动在后面添加空行
    pub fn database_info_with_model(
        &self,
        path: &Path,
        record_count: usize,
        model: &str,
        dimension: usize,
    ) {
        if self.silent {
            return;
        }
        eprintln!(
            "{:>12} {} {}",
            self.green.apply_to("Database"),
            path.display(),
            self.dim.apply_to(format!(
                "({} records, {}/{}d)",
                record_count, model, dimension
            ))
        );
        eprintln!();
    }

    /// 显示创建/查找资源消息
    /// 格式: "    Creating config at /path/to/config"
    /// 自动在后面添加空行
    pub fn resource_action(&self, action: &str, resource: &str, path: &Path) {
        if self.silent {
            return;
        }
        eprintln!(
            "{:>12} {} at {}",
            self.green.apply_to(action),
            resource,
            path.display()
        );
        eprintln!();
    }

    // === 结果显示方法 ===

    /// 显示搜索结果（引用格式，[N] score id date tags + 内容首行）
    pub fn search_results_brief(&self, results: &[QueryResult]) {
        println!();
        for (i, result) in results.iter().enumerate() {
            let score_part = if let Some(s) = result.score {
                let prefix = match result.score_type {
                    Some(ScoreType::Rerank) => "R",
                    Some(ScoreType::Vector) | None => "V",
                };
                self.green
                    .apply_to(format!("[{}:{:.2}]", prefix, s))
                    .to_string()
            } else {
                String::new()
            };

            let date = chrono::DateTime::from_timestamp_millis(result.updated_at)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "N/A".to_string());

            let tags_part = if result.tags.is_empty() {
                String::new()
            } else {
                format!(
                    " {}",
                    self.dim.apply_to(format!("[{}]", result.tags.join(", ")))
                )
            };

            // 首行内容，最多 100 个字符
            let first_line = result.content.lines().next().unwrap_or("").trim();
            let truncated: String = if first_line.chars().count() > 100 {
                format!("{}...", first_line.chars().take(100).collect::<String>())
            } else {
                first_line.to_string()
            };

            // 计算标题缩进：与 ID 列起始位置对齐
            // 格式："{[N]} {[S:0.00]} {id} ..."，ID 前的偏移 = [N].len + 1 + score.len + 1
            let index_len = format!("[{}]", i + 1).len();
            let score_plain_len = if result.score.is_some() {
                "[V:0.00]".len()
            } else {
                0
            };
            let id_offset = index_len + 1 + score_plain_len + 1;

            println!(
                "{} {} {} {}{}",
                self.dim.apply_to(format!("[{}]", i + 1)),
                score_part,
                self.bold.apply_to(&result.id),
                self.dim.apply_to(format!("({})", date)),
                tags_part
            );
            println!("{}{}", " ".repeat(id_offset), truncated);
        }
    }

    /// 显示列表结果（列表格式，不带分数）
    pub fn list_results(&self, results: &[QueryResult]) {
        for (i, result) in results.iter().enumerate() {
            // 创建一个不带分数的副本
            let mut list_result = result.clone();
            list_result.score = None;

            self.display_result_item_list(&list_result);

            // 只在非最后一个结果后添加空行分隔
            if i < results.len() - 1 {
                println!();
            }
        }
    }

    /// 流式打印 LLM 生成的综合回答，逐 token 输出到 stdout
    pub async fn llm_answer_stream(&self, stream: lmkit::ChatStream) {
        use futures::StreamExt;
        use std::io::Write;
        println!();
        let mut stream = stream;
        while let Some(chunk) = stream.next().await {
            if let Ok(c) = chunk {
                if let Some(delta) = c.delta {
                    print!("{}", delta);
                    let _ = std::io::stdout().flush();
                }
            }
        }
        println!("\n");
    }

    // === 消息提示方法 ===

    /// 显示提示消息（标准输出，右对齐）
    pub fn info(&self, message: &str) {
        println!("{:>12} {}", "", message);
    }

    /// 显示注意事项（右对齐）
    pub fn note(&self, message: &str) {
        if self.silent {
            return;
        }
        eprintln!("{:>12} {}", self.dim.apply_to("Note"), message);
    }

    /// 显示警告（黄色，右对齐）
    /// 自动在前后添加空行
    pub fn warning(&self, message: &str) {
        if self.silent {
            return;
        }
        eprintln!();
        eprintln!(
            "{:>12} {}",
            Style::new().yellow().bold().apply_to("Warning"),
            message
        );
        eprintln!();
    }

    /// 显示错误（红色，右对齐）
    ///
    /// 错误信息不受 `silent` 抑制，避免静默模式下丢失失败原因。
    pub fn error(&self, message: &str) {
        eprintln!(
            "{:>12} {}",
            Style::new().red().bold().apply_to("Error"),
            message
        );
    }

    /// 将 [`anyhow::Error`] 按 `{:#}` 格式化后通过 [`Output::error`] 输出（多行时首行带 Error 标签，其余行缩进）
    pub fn error_report(&self, err: &anyhow::Error) {
        self.error_report_str(&format!("{:#}", err));
    }

    /// 将整段错误文案按与 [`Output::error`] 一致的样式输出
    pub fn error_report_str(&self, message: &str) {
        let mut lines = message.lines();
        let Some(first) = lines.next() else {
            self.error("(unknown error)");
            return;
        };
        self.error(first);
        for line in lines {
            if line.is_empty() {
                continue;
            }
            eprintln!("{:>12} {}", "", self.dim.apply_to(line));
        }
    }

    /// 输出单行错误后返回 [`AlreadyReported`]，供 `main` 识别并避免重复打印
    pub fn fail(&self, message: &str) -> anyhow::Error {
        self.error(message);
        AlreadyReported.into()
    }

    // === 用户交互方法 ===

    /// 显示确认提示并读取用户输入
    /// 返回用户是否输入了 "yes"
    pub fn confirm(&self, expected: &str) -> io::Result<bool> {
        println!();
        print!(
            "{:>12} Type {} to confirm: ",
            "",
            Style::new().green().bold().apply_to(expected)
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim() == expected)
    }

    // === 私有辅助方法 ===

    /// 显示单个结果项（列表格式）
    /// 格式: "[0.89] id (date) [tag1, tag2]" 或 "id (date) [tag1, tag2]"
    ///       "       Content line 1"
    ///       "       Content line 2"
    fn display_result_item_list(&self, result: &QueryResult) {
        let id = &result.id;
        let content = &result.content;
        let tags = &result.tags;
        let score = result.score;

        let date = chrono::DateTime::from_timestamp_millis(result.updated_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "N/A".to_string());

        // 构建分数部分（如果有，用中括号括起来，带类型前缀）
        let score_part = if let Some(s) = score {
            let type_prefix = match result.score_type {
                Some(memo_types::ScoreType::Rerank) => "R:",
                Some(memo_types::ScoreType::Vector) => "V:",
                None => "",
            };
            format!(
                "{} ",
                self.green.apply_to(format!("[{}{:.2}]", type_prefix, s))
            )
        } else {
            String::new()
        };

        // 构建 tags 部分
        let tags_part = if tags.is_empty() {
            String::new()
        } else {
            format!(" {}", self.dim.apply_to(format!("[{}]", tags.join(", "))))
        };

        println!(
            "{}{} {}{}",
            score_part,
            self.bold.apply_to(id),
            self.dim.apply_to(format!("({})", date)),
            tags_part
        );

        // 计算缩进宽度：score_part(如果有) = "[R:0.89] " 或 "[V:0.89] " = 9个字符，否则0
        let indent_width = if score.is_some() { 9 } else { 0 };
        let indent = " ".repeat(indent_width);

        // 全文显示，每行保持与 ID 对齐的缩进
        for line in content.lines() {
            println!("{}{}", indent, line);
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}
