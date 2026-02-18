use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Paper {
    pub id: Option<i64>,
    pub title: String,
    pub title_zh: Option<String>,
    pub authors: Option<String>,
    pub abstract_text: Option<String>,
    pub abstract_zh: Option<String>,
    pub publish_date: Option<String>,
    pub source: String,
    pub source_id: String,
    pub pdf_url: Option<String>,
    pub pdf_path: Option<String>,
    pub processed: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExtractedContent {
    pub id: Option<i64>,
    pub paper_id: i64,
    pub formulas: Option<String>,
    pub images: Option<String>,
    pub tables: Option<String>,
    pub key_points: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Report {
    pub id: Option<i64>,
    pub subscription_id: Option<i64>,
    pub report_date: String,
    pub paper_count: Option<i64>,
    pub ppt_path: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
}
