use console::Style;
use std::path::Path;

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

    /// 完成状态消息（在同一行输出换行）
    #[allow(dead_code)]
    pub fn status_done(&self) {
        // 已废弃 - status() 现在直接换行
    }

    /// 显示数据库信息
    /// 格式: "    Database /path/to/db (123 records)"
    pub fn database_info(&self, path: &Path, record_count: usize) {
        eprintln!(
            "{:>12} {} {}",
            self.green.apply_to("Database"),
            path.display(),
            self.dim.apply_to(format!("({} records)", record_count))
        );
    }

    /// 显示数据库信息（带模型）
    /// 格式: "    Database /path/to/db (123 records, text-embedding-v4/1024d)"
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
    }

    /// 显示创建/查找资源消息
    /// 格式: "    Creating config at /path/to/config"
    pub fn resource_action(&self, action: &str, resource: &str, path: &Path) {
        eprintln!(
            "{:>12} {} at {}",
            self.green.apply_to(action),
            resource,
            path.display()
        );
    }

    /// 显示完成消息
    /// 格式: "    Finished action for scope"
    pub fn finish(&self, action: &str, scope: &str) {
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

    /// 显示搜索结果项
    /// 格式: "[0.89] Title (2024-01-22 14:30)"
    ///       "    Content preview..."
    pub fn search_result(&self, score: f32, title: &str, date: &str, content: &str) {
        println!(
            "[{}] {} {}",
            self.dim.apply_to(format!("{:.2}", score)),
            self.bold.apply_to(title),
            self.dim.apply_to(format!("({})", date))
        );

        let preview = self.format_content_preview(content);
        println!("    {}", self.dim.apply_to(preview));
        println!();
    }

    /// 显示列表项
    /// 格式: "[1/10] Title (2024-01-22)"
    ///       "    Content preview..."
    pub fn list_item(&self, index: usize, total: usize, title: &str, date: &str, content: &str) {
        println!(
            "[{}] {} {}",
            self.dim.apply_to(format!("{}/{}", index, total)),
            self.bold.apply_to(title),
            self.dim.apply_to(format!("({})", date))
        );

        let preview = self.format_content_preview(content);
        println!("    {}", self.dim.apply_to(preview));
        println!();
    }

    /// 格式化内容预览：过滤换行、截断到 100 字符
    fn format_content_preview(&self, content: &str) -> String {
        // 将换行符替换为空格
        let single_line = content.replace(['\n', '\r'], " ");
        let truncated: String = single_line.chars().take(100).collect();

        if single_line.len() > 100 {
            format!("{}...", truncated.trim_end())
        } else {
            truncated.trim().to_string()
        }
    }

    /// 显示注意事项（右对齐）
    pub fn note(&self, message: &str) {
        eprintln!("{:>12} {}", self.dim.apply_to("note:"), message);
    }

    /// 显示警告（红色，右对齐）
    pub fn warning(&self, message: &str) {
        eprintln!(
            "{:>12} {}",
            Style::new().red().bold().apply_to("Warning"),
            message
        );
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
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}
