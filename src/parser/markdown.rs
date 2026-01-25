use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;

use crate::models::{MemoMetadata, MemoSection};

pub fn parse_markdown_file(file_path: &Path) -> Result<Vec<MemoSection>> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    let sections = extract_sections(&content)?;

    Ok(sections)
}

fn extract_sections(content: &str) -> Result<Vec<MemoSection>> {
    let mut sections = Vec::new();

    // 提取 frontmatter (如果存在)
    let (metadata, content_without_frontmatter) = extract_frontmatter(content)?;

    // 匹配 ## 和 ### 标题
    let re = Regex::new(r"^(#{2,3})\s+(.+)$")?;

    let lines: Vec<&str> = content_without_frontmatter.lines().collect();
    let mut current_section_title = String::from("Overview");
    let mut current_content = String::new();

    for line in lines {
        if let Some(caps) = re.captures(line) {
            // 保存上一个章节
            if !current_content.is_empty() {
                sections.push(MemoSection {
                    section_title: current_section_title.clone(),
                    content: current_content.trim().to_string(),
                    metadata: metadata.clone(),
                });
            }
            current_section_title = caps[2].to_string();
            current_content.clear();
        } else {
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
        }
    }

    // 保存最后一个章节
    if !current_content.is_empty() {
        sections.push(MemoSection {
            section_title: current_section_title,
            content: current_content.trim().to_string(),
            metadata,
        });
    }

    Ok(sections)
}

fn extract_frontmatter(content: &str) -> Result<(MemoMetadata, String)> {
    let frontmatter_re = Regex::new(r"^---\s*\n(.*?)\n---\s*\n")?;

    if let Some(caps) = frontmatter_re.captures(content) {
        let frontmatter = caps.get(1).map_or("", |m| m.as_str());
        let remaining_content = &content[caps.get(0).unwrap().end()..];

        // 解析 frontmatter
        let mut tags = Vec::new();

        for line in frontmatter.lines() {
            let line = line.trim();

            // 解析 tags
            if line.starts_with("tags:") {
                let tags_str = line.strip_prefix("tags:").unwrap_or("").trim();
                // 支持两种格式: tags: [tag1, tag2] 或 tags: tag1, tag2
                if tags_str.starts_with('[') && tags_str.ends_with(']') {
                    let tags_str = &tags_str[1..tags_str.len() - 1];
                    tags = tags_str
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else {
                    tags = tags_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }

        Ok((MemoMetadata { tags }, remaining_content.to_string()))
    } else {
        // 没有 frontmatter
        Ok((MemoMetadata { tags: Vec::new() }, content.to_string()))
    }
}
