use console::Style;
use std::io::{self, Write};
use std::path::Path;

use crate::service::query::QueryResult;

/// 命令行输出格式化工具
/// 提供统一的 Cargo 风格输出
pub struct Output {
    green: Style,
    bold: Style,
    dim: Style,
}

impl Output {
    pub fn new() -> Self {
        Self {
            green: Style::new().green().bold(),
            bold: Style::new().bold(),
            dim: Style::new().dim(),
        }
    }

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

    /// 完成状态消息（在同一行输出换行）
    #[allow(dead_code)]
    pub fn status_done(&self) {
        // 已废弃 - status() 现在直接换行
    }

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

    /// 显示完成消息（简单版本）
    /// 格式: "    Finished action"
    #[allow(dead_code)]
    pub fn finish_simple(&self, action: &str) {
        eprintln!("{:>12} {}", self.green.apply_to("Finished"), action);
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

    /// 显示查询信息（右对齐）
    #[allow(dead_code)]
    pub fn query_info(&self, query: &str) {
        eprintln!("{:>12} {}", self.green.apply_to("Query"), query);
    }

    /// 显示单个结果项（统一格式）
    /// 格式: "[1/5] 0.89 id (date) [tag1, tag2]" 或 "[1/10] id (date) [tag1, tag2]"
    ///       "            Content line 1"
    ///       "            Content line 2"
    fn result_item(&self, index: usize, total: usize, result: &QueryResult) {
        let id = &result.id;
        let content = &result.content;
        let tags = &result.tags;
        let score = result.score;

        let date = chrono::DateTime::from_timestamp_millis(result.updated_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "N/A".to_string());
        // 构建索引部分
        let index_part = format!("{}/{}", index, total);

        // 构建分数部分（如果有）
        let score_part = if let Some(s) = score {
            format!(" {}", Style::new().green().apply_to(format!("{:.2}", s)))
        } else {
            String::new()
        };

        // 构建 tags 部分
        let tags_part = if let Some(tags) = tags {
            if tags.is_empty() {
                String::new()
            } else {
                format!(" {}", self.dim.apply_to(format!("[{}]", tags.join(", "))))
            }
        } else {
            String::new()
        };

        println!(
            "[{}]{} {} {}{}",
            self.dim.apply_to(&index_part),
            score_part,
            self.bold.apply_to(id),
            self.dim.apply_to(format!("({})", date)),
            tags_part
        );

        // 计算缩进宽度：[index_part] + score_part(如果有) + 空格
        // 由于 score_part 包含颜色代码，需要计算实际显示宽度
        let score_width = if score.is_some() { 5 } else { 0 }; // " 0.89" = 5个字符
        let indent_width = index_part.len() + 2 + score_width + 1; // [len] + score + 空格
        let indent = " ".repeat(indent_width);

        // 全文显示，每行保持与 ID 对齐的缩进
        for line in content.lines() {
            println!("{}{}", indent, line);
        }
    }

    /// 显示搜索结果列表（自动处理分隔和格式化）
    pub fn search_results(&self, results: &[QueryResult]) {
        let total = results.len();
        for (i, result) in results.iter().enumerate() {
            self.result_item(i + 1, total, result);

            // 只在非最后一个结果后添加空行分隔
            if i < results.len() - 1 {
                println!();
            }
        }
    }

    /// 显示列表（自动处理分隔和格式化）
    pub fn list_items(&self, results: &[QueryResult]) {
        let total = results.len();
        for (i, result) in results.iter().enumerate() {
            // 创建一个不带分数的副本
            let mut list_result = result.clone();
            list_result.score = None;

            self.result_item(i + 1, total, &list_result);

            // 只在非最后一个结果后添加空行分隔
            if i < results.len() - 1 {
                println!();
            }
        }
    }

    /// 显示注意事项（右对齐）
    pub fn note(&self, message: &str) {
        eprintln!("{:>12} {}", self.dim.apply_to("Note"), message);
    }

    /// 显示警告（黄色，右对齐）
    /// 自动在前面添加空行
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
    #[allow(dead_code)]
    pub fn error(&self, message: &str) {
        eprintln!(
            "{:>12} {}",
            Style::new().red().bold().apply_to("Error"),
            message
        );
    }

    /// 显示提示消息（标准输出，右对齐）
    pub fn info(&self, message: &str) {
        println!("{:>12} {}", "", message);
    }

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
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}
