use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivPaper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: String,
    pub published: String,
    pub pdf_url: String,
    pub categories: Vec<String>,
}

pub struct ArxivCrawler {
    client: Client,
    base_url: String,
    max_retries: u32,
}

impl ArxivCrawler {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("ResearchBot/1.0 (academic research; mailto:user@example.com)")
            .build()
            .unwrap();

        Self {
            client,
            base_url: "https://export.arxiv.org/api/query".to_string(),
            max_retries: 3,
        }
    }

    pub async fn search(&self, keywords: &[String], max_results: usize) -> Result<Vec<ArxivPaper>> {
        // 简化查询，只使用第一个关键词
        let query = keywords.first()
            .unwrap_or(&"machine learning".to_string())
            .replace(" ", "+");
        let url = format!(
            "{}?search_query=all:{}&start=0&max_results={}&sortBy=submittedDate&sortOrder=descending",
            self.base_url, query, max_results
        );

        info!("正在搜索 arXiv: {}", url);

        for attempt in 1..=self.max_retries {
            // 请求前延迟，arXiv 要求至少3秒间隔
            let delay = Duration::from_secs(3 * attempt as u64);
            info!("等待 {}s 后发送请求 (第 {}/{} 次)", delay.as_secs(), attempt, self.max_retries);
            tokio::time::sleep(delay).await;

            let response = match self.client.get(&url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("请求失败 (第 {}/{} 次): {}", attempt, self.max_retries, e);
                    continue;
                }
            };

            let status = response.status();
            let text = response.text().await?;

            info!("arXiv 响应状态: {}, 内容长度: {} 字节", status, text.len());

            // 429/502/503 或响应体含 "Rate exceeded" 都视为限流/服务不可用
            if status.as_u16() == 429 || status.as_u16() == 502 || status.as_u16() == 503
                || text.contains("Rate exceeded")
            {
                warn!("arXiv 返回 {} (第 {}/{} 次尝试)", status, attempt, self.max_retries);
                if attempt < self.max_retries {
                    let backoff = Duration::from_secs(30 * attempt as u64);
                    info!("等待 {}s 后重试...", backoff.as_secs());
                    tokio::time::sleep(backoff).await;
                }
                continue;
            }

            let papers = self.parse_arxiv_response(&text)?;
            info!("找到 {} 篇论文", papers.len());
            return Ok(papers);
        }

        warn!("arXiv API 请求在 {} 次重试后仍然失败", self.max_retries);
        Ok(vec![])
    }

    fn parse_arxiv_response(&self, xml: &str) -> Result<Vec<ArxivPaper>> {
        let mut papers = Vec::new();

        if !xml.contains("<entry>") {
            warn!("XML中没有找到<entry>标签");
            warn!("XML前500字符: {}", &xml.chars().take(500).collect::<String>());
            return Ok(papers);
        }

        for entry_text in xml.split("<entry>").skip(1) {
            if let Some(paper) = self.parse_entry(entry_text) {
                papers.push(paper);
            }
        }

        if papers.is_empty() {
            warn!("未能解析到论文，可能是XML格式问题");
        }

        Ok(papers)
    }

    fn parse_entry(&self, entry_text: &str) -> Option<ArxivPaper> {
        let id = self.extract_tag(entry_text, "id")?;

        let title = self.extract_tag(entry_text, "title")?
            .trim()
            .replace("\n", " ")
            .replace("  ", " ");

        let summary = self.extract_tag(entry_text, "summary")?
            .trim()
            .replace("\n", " ")
            .replace("  ", " ");

        let published = self.extract_tag(entry_text, "published")?;

        let mut authors = Vec::new();
        for author_block in entry_text.split("<author>").skip(1) {
            if let Some(name) = self.extract_tag(author_block, "name") {
                authors.push(name.trim().to_string());
            }
        }

        // 提取PDF链接
        let pdf_url = if let Some(pdf_id) = id.strip_prefix("http://arxiv.org/abs/") {
            format!("http://arxiv.org/pdf/{}.pdf", pdf_id)
        } else {
            format!("{}.pdf", id.replace("/abs/", "/pdf/"))
        };

        let mut categories = Vec::new();
        for cat_block in entry_text.split("<category term=\"").skip(1) {
            if let Some(end) = cat_block.find('"') {
                categories.push(cat_block[..end].to_string());
            }
        }

        Some(ArxivPaper {
            id: id.clone(),
            title,
            authors,
            summary,
            published,
            pdf_url,
            categories,
        })
    }

    fn extract_tag(&self, text: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);

        let start = text.find(&start_tag)? + start_tag.len();
        let end = text.find(&end_tag)?;

        Some(text[start..end].to_string())
    }

    pub async fn download_pdf(&self, url: &str, save_path: &str) -> Result<()> {
        info!("下载PDF: {} -> {}", url, save_path);

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            error!("下载失败，状态码: {}", response.status());
            return Err(anyhow::anyhow!("下载失败: {}", response.status()));
        }

        let bytes = response.bytes().await?;
        tokio::fs::write(save_path, bytes).await?;

        info!("PDF下载完成: {}", save_path);

        Ok(())
    }
}
