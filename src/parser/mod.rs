pub mod pdf_parser;
pub mod formula_extractor;
pub mod image_analyzer;
pub mod table_parser;

pub use pdf_parser::PdfParser;
pub use formula_extractor::FormulaExtractor;
pub use image_analyzer::ImageAnalyzer;
pub use table_parser::TableParser;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// 论文章节
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    pub level: u8,
    pub body: String,
}

/// 论文元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperMetadata {
    pub title: Option<String>,
    pub title_zh: Option<String>,
    pub authors: Vec<String>,
    pub abstract_text: Option<String>,
    pub abstract_zh: Option<String>,
}

/// 提取的公式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Formula {
    pub raw: String,
    pub context: String,
}

/// 提取的图片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedImage {
    pub filename: String,
    pub page: usize,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

/// 提取的表格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub caption: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// 聚合全部提取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperContent {
    pub metadata: PaperMetadata,
    pub sections: Vec<Section>,
    pub formulas: Vec<Formula>,
    pub images: Vec<ExtractedImage>,
    pub tables: Vec<Table>,
    pub full_text: String,
}

/// 统一提取管道
pub struct ExtractionPipeline {
    pdf_parser: PdfParser,
    formula_extractor: FormulaExtractor,
    image_analyzer: ImageAnalyzer,
    table_parser: TableParser,
}

impl ExtractionPipeline {
    pub fn new() -> Self {
        Self {
            pdf_parser: PdfParser::new(),
            formula_extractor: FormulaExtractor::new(),
            image_analyzer: ImageAnalyzer::new(),
            table_parser: TableParser::new(),
        }
    }

    /// 处理一篇论文的PDF，返回全部提取结果
    pub fn process(&self, pdf_path: &str, paper_id: &str, images_dir: &str) -> Result<PaperContent> {
        info!("开始提取管道: {}", pdf_path);

        // 1. 提取全文
        let full_text = self.pdf_parser.extract_full_text(pdf_path)?;

        // 2. 结构化文本提取
        let (metadata, sections) = self.pdf_parser.extract_structured_text(&full_text);
        info!("提取到 {} 个章节", sections.len());

        // 3. 公式提取
        let formulas = self.formula_extractor.extract(&full_text);
        info!("提取到 {} 个公式", formulas.len());

        // 4. 图片提取
        let images = match self.image_analyzer.extract_images(pdf_path, paper_id, images_dir) {
            Ok(imgs) => {
                info!("提取到 {} 张图片", imgs.len());
                imgs
            }
            Err(e) => {
                warn!("图片提取失败: {}", e);
                Vec::new()
            }
        };

        // 5. 表格解析
        let tables = self.table_parser.extract(&full_text);
        info!("提取到 {} 个表格", tables.len());

        Ok(PaperContent {
            metadata,
            sections,
            formulas,
            images,
            tables,
            full_text,
        })
    }
}
