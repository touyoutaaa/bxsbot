use anyhow::Result;
use tracing::{info, warn};
use std::path::Path;

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

        // 使用pdf-extract提取文本
        let text = pdf_extract::extract_text(pdf_path)?;

        // 按行分割
        let lines: Vec<String> = text
            .lines()
            .filter(|line| !line.trim().is_empty()) // 过滤空行
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
}
