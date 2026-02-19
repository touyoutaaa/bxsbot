use anyhow::Result;
use regex::Regex;
use tracing::{info, warn};
use std::path::Path;

use super::{Section, PaperMetadata};

pub struct PdfParser;

impl PdfParser {
    pub fn new() -> Self {
        Self
    }

    /// 提取PDF前N行文本
    pub fn extract_first_lines(&self, pdf_path: &str, num_lines: usize) -> Result<Vec<String>> {
        info!("解析PDF: {}", pdf_path);

        if !Path::new(pdf_path).exists() {
            return Err(anyhow::anyhow!("PDF文件不存在: {}", pdf_path));
        }

        let text = pdf_extract::extract_text(pdf_path)?;

        let lines: Vec<String> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(num_lines)
            .map(|s| s.trim().to_string())
            .collect();

        if lines.is_empty() {
            warn!("PDF中未提取到文本内容");
        } else {
            info!("成功提取 {} 行文本", lines.len());
        }

        Ok(lines)
    }

    /// 提取完整文本
    pub fn extract_full_text(&self, pdf_path: &str) -> Result<String> {
        info!("提取PDF完整文本: {}", pdf_path);

        if !Path::new(pdf_path).exists() {
            return Err(anyhow::anyhow!("PDF文件不存在: {}", pdf_path));
        }

        let text = pdf_extract::extract_text(pdf_path)?;
        info!("提取文本长度: {} 字符", text.len());

        Ok(text)
    }

    /// 结构化文本提取：识别章节标题，分割为 Section 列表，同时提取元数据
    pub fn extract_structured_text(&self, full_text: &str) -> (PaperMetadata, Vec<Section>) {
        let lines: Vec<&str> = full_text.lines().collect();

        // 章节标题模式
        let heading_patterns = [
            // "1. Introduction" or "1 Introduction"
            Regex::new(r"^(\d+)\.?\s+([A-Z][A-Za-z\s]+)$").unwrap(),
            // "1.1 Background" or "1.1. Background"
            Regex::new(r"^(\d+\.\d+)\.?\s+([A-Z][A-Za-z\s]+)$").unwrap(),
            // Known section names
            Regex::new(r"(?i)^(Abstract|Introduction|Related\s+Work|Methods?|Methodology|Experiments?|Results?|Discussion|Conclusion|Conclusions|Acknowledgments?|References|Appendix|Background)$").unwrap(),
        ];

        let mut sections: Vec<Section> = Vec::new();
        let mut current_heading = String::new();
        let mut current_level: u8 = 0;
        let mut current_body = String::new();

        // Extract title from first non-empty line
        let title = lines.iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string());

        let mut abstract_text: Option<String> = None;

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !current_body.is_empty() {
                    current_body.push('\n');
                }
                continue;
            }

            let mut matched_heading = false;

            // Check numbered heading "1. Title" or "1 Title"
            if let Some(caps) = heading_patterns[0].captures(trimmed) {
                Self::push_section(&mut sections, &current_heading, current_level, &current_body);
                current_heading = trimmed.to_string();
                current_level = 1;
                current_body.clear();
                matched_heading = true;
                let _ = caps;
            }
            // Check sub-heading "1.1 Title"
            else if let Some(caps) = heading_patterns[1].captures(trimmed) {
                Self::push_section(&mut sections, &current_heading, current_level, &current_body);
                current_heading = trimmed.to_string();
                current_level = 2;
                current_body.clear();
                matched_heading = true;
                let _ = caps;
            }
            // Check known section names
            else if heading_patterns[2].is_match(trimmed) {
                Self::push_section(&mut sections, &current_heading, current_level, &current_body);
                current_heading = trimmed.to_string();
                current_level = 1;
                current_body.clear();
                matched_heading = true;
            }

            if !matched_heading {
                if !current_body.is_empty() {
                    current_body.push(' ');
                }
                current_body.push_str(trimmed);
            }
        }

        // Push last section
        Self::push_section(&mut sections, &current_heading, current_level, &current_body);

        // Extract abstract from sections
        if let Some(abs_section) = sections.iter().find(|s| s.heading.to_lowercase() == "abstract") {
            abstract_text = Some(abs_section.body.clone());
        }

        let metadata = PaperMetadata {
            title,
            title_zh: None,
            authors: Vec::new(), // Author extraction from PDF text is unreliable
            abstract_text,
            abstract_zh: None,
        };

        (metadata, sections)
    }

    fn push_section(sections: &mut Vec<Section>, heading: &str, level: u8, body: &str) {
        let body_trimmed = body.trim();
        if heading.is_empty() && body_trimmed.is_empty() {
            return;
        }
        sections.push(Section {
            heading: if heading.is_empty() { "(untitled)".to_string() } else { heading.to_string() },
            level,
            body: body_trimmed.to_string(),
        });
    }
}
