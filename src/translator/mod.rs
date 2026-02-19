use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::TranslatorConfig;

/// MiniMax API 请求体
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// MiniMax API 响应体
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

pub struct Translator {
    client: reqwest::Client,
    config: TranslatorConfig,
}

impl Translator {
    pub fn new(config: TranslatorConfig) -> Self {
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60));

        if !config.proxy.is_empty() {
            match reqwest::Proxy::all(&config.proxy) {
                Ok(proxy) => {
                    info!("使用代理: {}", config.proxy);
                    builder = builder.proxy(proxy);
                }
                Err(e) => {
                    warn!("代理配置无效 '{}': {}", config.proxy, e);
                }
            }
        }

        let client = builder.build().expect("Failed to create HTTP client");
        Self { client, config }
    }

    /// 检查 API key 是否已配置
    pub fn is_configured(&self) -> bool {
        !self.config.api_key.is_empty()
            && self.config.api_key != "your-api-key"
    }

    /// 翻译单段文本
    pub async fn translate_text(&self, text: &str, context: &str) -> Result<String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let system_prompt = format!(
            "你是一位专业的学术翻译专家。请将以下英文学术{context}翻译为中文。\n\
             翻译要求：\n\
             1. 保持学术风格，翻译准确流畅\n\
             2. 专业术语保留英文原文（用括号标注），如：卷积神经网络（CNN）\n\
             3. 不要翻译LaTeX公式、数学符号、人名\n\
             4. 不要添加任何解释，只输出翻译结果",
            context = context
        );

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: 0.3,
        };

        self.call_api(&request).await
    }

    /// 翻译论文标题和摘要（单次 API 调用）
    pub async fn translate_paper(&self, title: &str, abstract_text: &str) -> Result<(String, String)> {
        let system_prompt = "你是一位专业的学术翻译专家。请将英文学术论文的标题和摘要翻译为中文。\n\
             翻译要求：\n\
             1. 保持学术风格，翻译准确流畅\n\
             2. 专业术语保留英文原文（用括号标注），如：卷积神经网络（CNN）\n\
             3. 不要翻译LaTeX公式、数学符号、人名\n\
             4. 请严格按以下格式输出，不要添加其他内容：\n\
             [标题翻译]\n\
             翻译后的标题\n\
             [摘要翻译]\n\
             翻译后的摘要";

        let user_content = format!(
            "请翻译以下论文：\n\n标题：{title}\n\n摘要：{abstract_text}",
            title = title,
            abstract_text = abstract_text,
        );

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            temperature: 0.3,
        };

        let response = self.call_api(&request).await?;

        // 解析结构化响应
        let (title_zh, abstract_zh) = parse_translation_response(&response, title);
        Ok((title_zh, abstract_zh))
    }

    /// 调用 MiniMax API，带重试逻辑
    async fn call_api(&self, request: &ChatRequest) -> Result<String> {
        let mut last_error = None;

        for attempt in 0..3 {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt as u32));
                info!("API 重试 ({}/3)，等待 {}ms...", attempt + 1, delay.as_millis());
                tokio::time::sleep(delay).await;
            }

            match self.do_request(request).await {
                Ok(content) => {
                    // 速率限制：每次调用后等待 500ms
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    return Ok(content);
                }
                Err(e) => {
                    warn!("API 调用失败 (尝试 {}/3): {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("API 调用失败")))
    }

    async fn do_request(&self, request: &ChatRequest) -> Result<String> {
        let response = self
            .client
            .post(&self.config.api_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .context("发送请求失败")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("API 返回错误 {}: {}", status, body);
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("解析 API 响应失败")?;

        let content = chat_response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        Ok(content)
    }
}

/// 解析 translate_paper 的结构化响应
fn parse_translation_response(response: &str, fallback_title: &str) -> (String, String) {
    let response = response.trim();

    // 尝试按 [标题翻译] / [摘要翻译] 标记分割
    if let (Some(title_start), Some(abstract_start)) = (
        response.find("[标题翻译]"),
        response.find("[摘要翻译]"),
    ) {
        let title_zh = response[title_start + "[标题翻译]".len()..abstract_start]
            .trim()
            .to_string();
        let abstract_zh = response[abstract_start + "[摘要翻译]".len()..]
            .trim()
            .to_string();

        if !title_zh.is_empty() && !abstract_zh.is_empty() {
            return (title_zh, abstract_zh);
        }
    }

    // 备选方案：如果格式不对，将整个响应作为摘要翻译，标题单独处理
    warn!("翻译响应格式不符预期，使用整体响应");
    (
        format!("{} (翻译失败，请重试)", fallback_title),
        response.to_string(),
    )
}
