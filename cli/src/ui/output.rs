use console::Style;
use std::io::{self, Write};
use std::path::Path;

use memo_types::QueryResult;

/// 命令行输出格式化工具
/// 提供统一的 Cargo 风格输出
pub struct Output {
    green: Style,
    bold: Style,
    dim: Style,
}

impl Output {
    // === 构造方法 ===

    pub fn new() -> Self {
        Self {
            green: Style::new().green().bold(),
            bold: Style::new().bold(),
            dim: Style::new().dim(),
        }
    }

    // === 状态和进度显示方法 ===

    /// 显示状态消息（如 "Loading model", "Embedding text" 等）
    /// 格式: "     Loading model ..."（动词右对齐到 12 字符）
    pub fn status(&self, action: &str, target: &str) {
        eprintln!("{:>12} {}", self.green.apply_to(action), target);
    }

    /// 开始执行操作的状态消息（会在前面自动添加空行）
    /// 用于标记一个新操作的开始，例如用户确认后的实际执行
    pub fn begin_operation(&self, action: &str, target: &str) {
        eprintln!();
        eprintln!("{:>12} {}", self.green.apply_to(action), target);
    }

    /// 显示完成消息
    /// 格式: "    Finished action for scope"
    /// 自动在前面添加空行
    pub fn finish(&self, action: &str, scope: &str) {
        eprintln!();
        eprintln!(
            "{:>12} {} for {} scope",
            self.green.apply_to("Finished"),
            action,
            scope
        );
    }

    // === 信息显示方法 ===

    /// 显示数据库信息
    /// 格式: "    Database /path/to/db (123 records)"
    /// 自动在后面添加空行
    pub fn database_info(&self, path: &Path, record_count: usize) {
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
        eprintln!(
            "{:>12} {} at {}",
            self.green.apply_to(action),
            resource,
            path.display()
        );
        eprintln!();
    }

    /// 显示统计信息
    /// 格式: "             12 files, 45 sections"
    pub fn stats(&self, items: &[(&str, usize)]) {
        let parts: Vec<String> = items
            .iter()
            .map(|(name, count)| format!("{} {}", count, name))
            .collect();
        eprintln!("{:>12} {}", "", self.dim.apply_to(parts.join(", ")));
    }

    // === 结果显示方法 ===

    /// 显示搜索结果（列表格式，带相似度分数）
    pub fn search_results(&self, results: &[QueryResult]) {
        for (i, result) in results.iter().enumerate() {
            self.display_result_item_list(result);

            // 只在非最后一个结果后添加空行分隔
            if i < results.len() - 1 {
                println!();
            }
        }
    }

    /// 显示搜索结果（带汇总信息）
    pub fn search_results_with_summary(&self, results: &[QueryResult], used_rerank: bool) {
        // 显示结果汇总
        let rank_method = if used_rerank {
            "rerank scores"
        } else {
            "vector scores"
        };

        eprintln!(
            "{:>12} {} results ranked by {}",
            self.green.apply_to("Results"),
            results.len(),
            rank_method
        );
        eprintln!();

        // 显示结果列表
        for (i, result) in results.iter().enumerate() {
            self.display_result_item_list(result);

            // 只在非最后一个结果后添加空行分隔
            if i < results.len() - 1 {
                println!();
            }
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

    // === 消息提示方法 ===

    /// 显示提示消息（标准输出，右对齐）
    pub fn info(&self, message: &str) {
        println!("{:>12} {}", "", message);
    }

    /// 显示注意事项（右对齐）
    pub fn note(&self, message: &str) {
        eprintln!("{:>12} {}", self.dim.apply_to("Note"), message);
    }

    /// 显示警告（黄色，右对齐）
    /// 自动在前后添加空行
    pub fn warning(&self, message: &str) {
        eprintln!();
        eprintln!(
            "{:>12} {}",
            Style::new().yellow().bold().apply_to("Warning"),
            message
        );
        eprintln!();
    }

    /// 显示错误（红色，右对齐）
    pub fn error(&self, message: &str) {
        eprintln!(
            "{:>12} {}",
            Style::new().red().bold().apply_to("Error"),
            message
        );
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
