mod config;
mod crawler;
mod parser;
mod translator;
mod generator;
mod storage;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

use config::{AppConfig, KeywordConfig};
use storage::Database;
use translator::Translator;
use utils::logger;

#[derive(Parser)]
#[command(name = "bsxbot")]
#[command(about = "科研信息自动提取与分析系统", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化配置和数据库
    Init,
    /// 运行爬虫任务
    Crawl {
        /// 订阅名称
        #[arg(short, long)]
        subscription: Option<String>,
    },
    /// 启动定时任务
    Schedule,
    /// 生成报告
    Report {
        /// 报告日期 (YYYY-MM-DD)
        #[arg(short, long)]
        date: Option<String>,
    },
    /// 翻译未翻译的论文
    Translate {
        /// 指定论文ID翻译
        #[arg(long)]
        id: Option<i64>,
    },
    /// 清理所有缓存数据
    Clean,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    logger::init_logger();
    info!("bsxbot 启动");

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            init_command().await?;
        }
        Commands::Crawl { subscription } => {
            crawl_command(subscription).await?;
        }
        Commands::Schedule => {
            schedule_command().await?;
        }
        Commands::Report { date } => {
            report_command(date).await?;
        }
        Commands::Translate { id } => {
            translate_command(id).await?;
        }
        Commands::Clean => {
            clean_command().await?;
        }
    }

    Ok(())
}

async fn init_command() -> Result<()> {
    info!("初始化系统...");

    // 创建必要的目录
    tokio::fs::create_dir_all("data/papers").await?;
    tokio::fs::create_dir_all("data/images").await?;
    tokio::fs::create_dir_all("data/reports").await?;
    tokio::fs::create_dir_all("config").await?;

    // 生成默认配置文件
    let app_config = AppConfig::default();
    app_config.save("config/settings.toml")?;
    info!("已生成配置文件: config/settings.toml");

    let keyword_config = KeywordConfig::default();
    let keyword_toml = toml::to_string_pretty(&keyword_config)?;
    tokio::fs::write("config/keywords.toml", keyword_toml).await?;
    info!("已生成关键词配置: config/keywords.toml");

    // 初始化数据库（确保data目录已创建）
    let db_path = "sqlite:./data/papers.db";
    info!("正在初始化数据库: {}", db_path);
    let db = Database::new(db_path).await?;
    db.init_schema().await?;
    info!("数据库初始化完成");

    info!("✅ 系统初始化完成！");
    info!("下一步:");
    info!("  1. 编辑 config/settings.toml 配置API密钥");
    info!("  2. 编辑 config/keywords.toml 配置研究方向");
    info!("  3. 运行 'bsxbot crawl' 开始爬取");

    Ok(())
}

async fn crawl_command(subscription: Option<String>) -> Result<()> {
    info!("开始爬取任务...");

    let app_config = AppConfig::load()?;
    let keyword_config = KeywordConfig::load()?;
    let db = Database::new(&format!("sqlite:{}", app_config.storage.database_path)).await?;

    // 初始化翻译器
    let translator = Translator::new(app_config.translator.clone());
    let translation_enabled = translator.is_configured();
    if !translation_enabled {
        info!("⚠️ API key 未配置，跳过翻译。请在 config/settings.toml 中设置 api_key");
    }

    let subscriptions = keyword_config.get_active_subscriptions();

    if subscriptions.is_empty() {
        info!("没有启用的订阅，请检查 config/keywords.toml");
        return Ok(());
    }

    for sub in subscriptions {
        if let Some(ref name) = subscription {
            if &sub.name != name {
                continue;
            }
        }

        info!("处理订阅: {}", sub.name);
        info!("关键词: {:?}", sub.keywords);

        // 使用 arXiv 爬虫
        if sub.sources.contains(&"arxiv".to_string()) {
            let crawler = crawler::ArxivCrawler::new();

            let papers = match crawler.search(&sub.keywords, app_config.crawler.max_papers_per_day).await {
                Ok(papers) => papers,
                Err(e) => {
                    info!("arXiv 搜索失败: {}", e);
                    continue;
                }
            };

            if papers.is_empty() {
                info!("未找到匹配的论文，跳过该订阅");
                continue;
            }

            info!("找到 {} 篇论文", papers.len());

            for paper in papers.iter().take(3) {
                info!("---");
                info!("标题: {}", paper.title);
                info!("作者: {}", paper.authors.join(", "));
                info!("发布日期: {}", paper.published);
                info!("PDF: {}", paper.pdf_url);

                // 提取arXiv ID
                let arxiv_id = paper.id.replace("http://arxiv.org/abs/", "");

                // 检查是否已存在
                if db.paper_exists("arxiv", &arxiv_id).await? {
                    info!("论文已存在，跳过");
                    continue;
                }

                // 保存到数据库
                let db_paper = storage::models::Paper {
                    id: None,
                    title: paper.title.clone(),
                    title_zh: None,
                    authors: Some(paper.authors.join(", ")),
                    abstract_text: Some(paper.summary.clone()),
                    abstract_zh: None,
                    publish_date: Some(paper.published.clone()),
                    source: "arxiv".to_string(),
                    source_id: arxiv_id.clone(),
                    pdf_url: Some(paper.pdf_url.clone()),
                    pdf_path: None,
                    processed: false,
                    created_at: None,
                };

                let paper_id = db.save_paper(&db_paper).await?;
                info!("论文已保存到数据库，ID: {}", paper_id);

                // 翻译标题和摘要
                if translation_enabled {
                    info!("正在翻译论文...");
                    match translator.translate_paper(&paper.title, &paper.summary).await {
                        Ok((title_zh, abstract_zh)) => {
                            db.update_translation("arxiv", &arxiv_id, &title_zh, &abstract_zh).await?;
                            info!("翻译完成: {}", title_zh);
                        }
                        Err(e) => {
                            info!("翻译失败: {}，继续处理", e);
                        }
                    }
                }

                // 下载PDF
                let pdf_filename = format!("data/papers/{}.pdf", arxiv_id.replace("/", "_"));
                match crawler.download_pdf(&paper.pdf_url, &pdf_filename).await {
                    Ok(_) => {
                        // 更新PDF路径
                        db.update_pdf_path("arxiv", &arxiv_id, &pdf_filename).await?;

                        // 使用提取管道解析PDF
                        let arxiv_id_safe = arxiv_id.replace("/", "_");
                        let pipeline = parser::ExtractionPipeline::new();
                        match pipeline.process(&pdf_filename, &arxiv_id_safe, "data/images") {
                            Ok(content) => {
                                info!("PDF解析完成:");
                                if let Some(ref title) = content.metadata.title {
                                    info!("  标题: {}", title);
                                }
                                if let Some(ref abs) = content.metadata.abstract_text {
                                    let preview = if abs.len() > 100 { &abs[..100] } else { abs };
                                    info!("  摘要: {}...", preview);
                                }
                                info!("  章节数: {}", content.sections.len());
                                info!("  公式数: {}", content.formulas.len());
                                info!("  图片数: {}", content.images.len());
                                info!("  表格数: {}", content.tables.len());

                                // 序列化存入数据库
                                let formulas_json = serde_json::to_string(&content.formulas).unwrap_or_default();
                                let images_json = serde_json::to_string(&content.images).unwrap_or_default();
                                let tables_json = serde_json::to_string(&content.tables).unwrap_or_default();
                                let sections_json = serde_json::to_string(&content.sections).unwrap_or_default();

                                if let Err(e) = db.save_extracted_content(
                                    paper_id,
                                    &formulas_json,
                                    &images_json,
                                    &tables_json,
                                    &sections_json,
                                ).await {
                                    info!("保存提取内容失败: {}", e);
                                }

                                // 标记论文已处理
                                db.mark_paper_processed("arxiv", &arxiv_id).await?;
                            }
                            Err(e) => {
                                info!("PDF解析失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        info!("PDF下载失败: {}", e);
                    }
                }

                // 延迟避免请求过快
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    app_config.crawler.request_delay_ms,
                ))
                .await;
            }
        }
    }

    info!("✅ 爬取任务完成");
    Ok(())
}

async fn translate_command(paper_id: Option<i64>) -> Result<()> {
    info!("开始翻译任务...");

    let app_config = AppConfig::load()?;
    let db = Database::new(&format!("sqlite:{}", app_config.storage.database_path)).await?;
    let translator = Translator::new(app_config.translator.clone());

    if !translator.is_configured() {
        info!("❌ API key 未配置。请在 config/settings.toml 中设置 [translator] api_key");
        return Ok(());
    }

    let papers = if let Some(_id) = paper_id {
        // 获取所有论文，过滤指定ID
        let all = db.get_all_papers().await?;
        all.into_iter().filter(|p| p.id == Some(_id)).collect::<Vec<_>>()
    } else {
        db.get_untranslated_papers().await?
    };

    if papers.is_empty() {
        info!("没有需要翻译的论文");
        return Ok(());
    }

    info!("找到 {} 篇待翻译论文", papers.len());

    let mut success_count = 0;
    let mut fail_count = 0;

    for paper in &papers {
        let abstract_text = paper.abstract_text.as_deref().unwrap_or("");
        if abstract_text.is_empty() {
            info!("论文 [{}] {} 没有摘要，跳过", paper.source_id, paper.title);
            continue;
        }

        info!("翻译: {}", paper.title);
        match translator.translate_paper(&paper.title, abstract_text).await {
            Ok((title_zh, abstract_zh)) => {
                db.update_translation(&paper.source, &paper.source_id, &title_zh, &abstract_zh).await?;
                info!("  ✅ {}", title_zh);
                success_count += 1;
            }
            Err(e) => {
                info!("  ❌ 翻译失败: {}", e);
                fail_count += 1;
            }
        }
    }

    info!("✅ 翻译完成: {} 成功, {} 失败", success_count, fail_count);
    Ok(())
}

async fn clean_command() -> Result<()> {
    info!("开始清理缓存数据...");

    let mut total_files = 0u64;

    // 清理 data/ 下的三个子目录
    for dir in &["data/papers", "data/images", "data/reports"] {
        match tokio::fs::read_dir(dir).await {
            Ok(mut entries) => {
                let mut count = 0u64;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.is_file() {
                        if let Err(e) = tokio::fs::remove_file(&path).await {
                            info!("删除失败 {}: {}", path.display(), e);
                        } else {
                            count += 1;
                        }
                    }
                }
                info!("已清理 {}: {} 个文件", dir, count);
                total_files += count;
            }
            Err(_) => {
                info!("目录不存在，跳过: {}", dir);
            }
        }
    }

    // 清空数据库表
    let app_config = AppConfig::load();
    match app_config {
        Ok(config) => {
            let db_url = format!("sqlite:{}", config.storage.database_path);
            match Database::new(&db_url).await {
                Ok(db) => {
                    db.clear_all_tables().await?;
                }
                Err(e) => {
                    info!("数据库连接失败，跳过清空: {}", e);
                }
            }
        }
        Err(_) => {
            info!("配置文件未找到，跳过数据库清空");
        }
    }

    info!("✅ 清理完成，共删除 {} 个文件", total_files);
    Ok(())
}

async fn schedule_command() -> Result<()> {
    info!("启动定时任务调度器...");

    let scheduler = utils::scheduler::TaskScheduler::new().await?;

    // 添加每日任务（每天早上8点执行）
    let job_fn = std::sync::Arc::new(|| {
        info!("执行每日爬取任务");
        // TODO: 调用爬取逻辑
    });

    scheduler
        .add_daily_job("0 0 8 * * *", job_fn)
        .await?;

    scheduler.start().await?;

    info!("调度器运行中，按 Ctrl+C 停止");

    // 保持运行
    tokio::signal::ctrl_c().await?;
    info!("收到停止信号");

    scheduler.shutdown().await?;
    Ok(())
}

async fn report_command(date: Option<String>) -> Result<()> {
    let report_date = date.unwrap_or_else(|| {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    });

    info!("生成报告: {}", report_date);

    let app_config = AppConfig::load()?;
    let db = Database::new(&format!("sqlite:{}", app_config.storage.database_path)).await?;

    // 从数据库获取论文翻译信息
    let db_papers = db.get_all_papers().await?;
    let translations: std::collections::HashMap<String, (Option<String>, Option<String>)> = db_papers
        .into_iter()
        .filter_map(|p| {
            let key = p.source_id.replace("/", "_");
            if p.title_zh.is_some() || p.abstract_zh.is_some() {
                Some((key, (p.title_zh, p.abstract_zh)))
            } else {
                None
            }
        })
        .collect();

    // Scan all PDFs in data/papers/
    let mut pdf_files: Vec<String> = Vec::new();
    let mut entries = tokio::fs::read_dir("data/papers").await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "pdf").unwrap_or(false) {
            pdf_files.push(path.to_string_lossy().to_string());
        }
    }

    if pdf_files.is_empty() {
        info!("data/papers/ 中没有PDF文件，请先运行 crawl");
        return Ok(());
    }

    pdf_files.sort();
    info!("找到 {} 个PDF文件", pdf_files.len());

    let pipeline = parser::ExtractionPipeline::new();
    let mut all_contents: Vec<(String, parser::PaperContent)> = Vec::new();

    for pdf_path in &pdf_files {
        let paper_id = std::path::Path::new(pdf_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        info!("处理: {}", paper_id);
        match pipeline.process(pdf_path, &paper_id, "data/images") {
            Ok(mut content) => {
                // 注入数据库中的翻译
                if let Some((title_zh, abstract_zh)) = translations.get(&paper_id) {
                    content.metadata.title_zh = title_zh.clone();
                    content.metadata.abstract_zh = abstract_zh.clone();
                }
                all_contents.push((paper_id, content));
            }
            Err(e) => {
                info!("处理 {} 失败: {}", pdf_path, e);
            }
        }
    }

    // Generate HTML
    let html = generate_html_report(&report_date, &all_contents);
    let output_path = format!("data/reports/report_{}.html", report_date);
    tokio::fs::create_dir_all("data/reports").await?;
    tokio::fs::write(&output_path, html).await?;

    info!("✅ 报告已生成: {}", output_path);
    Ok(())
}

fn generate_html_report(date: &str, papers: &[(String, parser::PaperContent)]) -> String {
    let mut html = format!(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>科研论文提取报告 - {date}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ font-family: -apple-system, "Segoe UI", Roboto, "Noto Sans SC", sans-serif; background: #f5f5f5; color: #333; line-height: 1.6; }}
.container {{ max-width: 1100px; margin: 0 auto; padding: 20px; }}
header {{ background: linear-gradient(135deg, #1a237e 0%, #283593 100%); color: white; padding: 40px 30px; border-radius: 12px; margin-bottom: 30px; }}
header h1 {{ font-size: 28px; margin-bottom: 8px; }}
header .meta {{ opacity: 0.85; font-size: 14px; }}
.paper {{ background: white; border-radius: 12px; padding: 30px; margin-bottom: 24px; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }}
.paper-title {{ font-size: 22px; color: #1a237e; margin-bottom: 8px; padding-bottom: 12px; border-bottom: 2px solid #e8eaf6; }}
.paper-title-zh {{ font-size: 18px; color: #37474f; margin-bottom: 16px; }}
.paper-id {{ font-size: 13px; color: #888; font-weight: normal; }}
.stats {{ display: flex; gap: 16px; margin-bottom: 20px; flex-wrap: wrap; }}
.stat {{ background: #f5f5f5; padding: 8px 16px; border-radius: 8px; font-size: 14px; }}
.stat b {{ color: #1a237e; }}
h3 {{ font-size: 17px; color: #283593; margin: 24px 0 12px 0; padding-left: 12px; border-left: 4px solid #5c6bc0; }}
.section {{ background: #fafafa; border-radius: 8px; padding: 16px; margin-bottom: 12px; }}
.section-heading {{ font-weight: 600; color: #37474f; margin-bottom: 6px; }}
.section-body {{ font-size: 14px; color: #555; white-space: pre-wrap; word-break: break-word; max-height: 300px; overflow-y: auto; }}
.translation {{ background: #e8f5e9; border-left: 3px solid #4caf50; padding: 12px 16px; margin-top: 8px; border-radius: 0 8px 8px 0; font-size: 14px; color: #2e7d32; }}
.translation-label {{ font-size: 12px; color: #66bb6a; margin-bottom: 4px; font-weight: 600; }}
.formula-list {{ list-style: none; }}
.formula-item {{ background: #fff8e1; border-left: 3px solid #ffc107; padding: 10px 14px; margin-bottom: 8px; border-radius: 0 6px 6px 0; font-family: "Cambria Math", "Latin Modern Math", Georgia, serif; font-size: 15px; word-break: break-all; }}
.formula-context {{ font-size: 12px; color: #888; margin-top: 4px; font-family: sans-serif; }}
.images-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px; }}
.image-card {{ background: #f5f5f5; border-radius: 8px; overflow: hidden; }}
.image-card img {{ width: 100%; height: auto; display: block; }}
.image-card .caption {{ padding: 8px 12px; font-size: 12px; color: #666; }}
table.data-table {{ width: 100%; border-collapse: collapse; margin-bottom: 12px; font-size: 14px; }}
table.data-table th {{ background: #e8eaf6; padding: 8px 12px; text-align: left; border: 1px solid #c5cae9; }}
table.data-table td {{ padding: 8px 12px; border: 1px solid #e0e0e0; }}
table.data-table tr:nth-child(even) {{ background: #fafafa; }}
.table-caption {{ font-size: 13px; color: #666; margin-bottom: 6px; font-style: italic; }}
.empty {{ color: #999; font-style: italic; padding: 12px; }}
</style>
</head>
<body>
<div class="container">
<header>
  <h1>科研论文提取报告</h1>
  <div class="meta">日期: {date} &nbsp;|&nbsp; 论文数: {count}</div>
</header>
"#, date = date, count = papers.len());

    for (paper_id, content) in papers {
        let title = content.metadata.title.as_deref().unwrap_or("(未提取到标题)");

        html.push_str(&format!(r#"<div class="paper">
<div class="paper-title">{title} <span class="paper-id">[{paper_id}]</span></div>
"#,
            title = html_escape(title),
            paper_id = html_escape(paper_id),
        ));

        // 中文标题
        if let Some(ref title_zh) = content.metadata.title_zh {
            if !title_zh.is_empty() {
                html.push_str(&format!(
                    r#"<div class="paper-title-zh">{}</div>"#,
                    html_escape(title_zh)
                ));
                html.push('\n');
            }
        }

        html.push_str(&format!(r#"<div class="stats">
  <div class="stat"><b>{sections}</b> 章节</div>
  <div class="stat"><b>{formulas}</b> 公式</div>
  <div class="stat"><b>{images}</b> 图片</div>
  <div class="stat"><b>{tables}</b> 表格</div>
</div>
"#,
            sections = content.sections.len(),
            formulas = content.formulas.len(),
            images = content.images.len(),
            tables = content.tables.len(),
        ));

        // Abstract
        if let Some(ref abs) = content.metadata.abstract_text {
            if !abs.is_empty() {
                html.push_str("<h3>摘要</h3>\n");
                html.push_str(&format!(r#"<div class="section"><div class="section-body">{}</div></div>"#,
                    html_escape(abs)));
                html.push('\n');

                // 中文摘要
                if let Some(ref abs_zh) = content.metadata.abstract_zh {
                    if !abs_zh.is_empty() {
                        html.push_str(&format!(
                            r#"<div class="translation"><div class="translation-label">中文翻译</div>{}</div>"#,
                            html_escape(abs_zh)
                        ));
                        html.push('\n');
                    }
                }
            }
        }

        // Sections
        if !content.sections.is_empty() {
            html.push_str("<h3>章节内容</h3>\n");
            for section in &content.sections {
                let body_preview = if section.body.len() > 800 {
                    format!("{}...", &section.body[..section.body.floor_char_boundary(800)])
                } else {
                    section.body.clone()
                };
                html.push_str(&format!(
                    r#"<div class="section"><div class="section-heading">{heading}</div><div class="section-body">{body}</div></div>"#,
                    heading = html_escape(&section.heading),
                    body = html_escape(&body_preview),
                ));
                html.push('\n');
            }
        }

        // Formulas
        if !content.formulas.is_empty() {
            html.push_str(&format!("<h3>公式 ({})</h3>\n", content.formulas.len()));
            html.push_str(r#"<ul class="formula-list">"#);
            let max_show = 30;
            for (i, formula) in content.formulas.iter().enumerate() {
                if i >= max_show {
                    html.push_str(&format!(
                        r#"<li class="formula-item" style="background:#f5f5f5">... 还有 {} 个公式未显示</li>"#,
                        content.formulas.len() - max_show));
                    break;
                }
                let raw_display = if formula.raw.len() > 200 {
                    format!("{}...", &formula.raw[..formula.raw.floor_char_boundary(200)])
                } else {
                    formula.raw.clone()
                };
                html.push_str(&format!(
                    r#"<li class="formula-item">{raw}<div class="formula-context">...{ctx}...</div></li>"#,
                    raw = html_escape(&raw_display),
                    ctx = html_escape(&formula.context[..formula.context.len().min(120)]),
                ));
                html.push('\n');
            }
            html.push_str("</ul>\n");
        }

        // Images
        if !content.images.is_empty() {
            html.push_str(&format!("<h3>图片 ({})</h3>\n", content.images.len()));
            html.push_str(r#"<div class="images-grid">"#);
            let max_images = 20;
            for (i, img) in content.images.iter().enumerate() {
                if i >= max_images {
                    html.push_str(&format!(
                        r#"<div class="image-card"><div class="caption">... 还有 {} 张图片未显示</div></div>"#,
                        content.images.len() - max_images));
                    break;
                }
                // Convert path to relative from report location
                let img_path = img.filename.replace('\\', "/");
                // Report is at data/reports/, images at data/images/
                let relative_path = if img_path.starts_with("data/") {
                    format!("../{}", &img_path[5..])
                } else {
                    img_path.clone()
                };
                html.push_str(&format!(
                    r#"<div class="image-card"><img src="{src}" alt="page {page}" loading="lazy"><div class="caption">Page {page} &nbsp; {w}x{h} &nbsp; {fmt}</div></div>"#,
                    src = html_escape(&relative_path),
                    page = img.page,
                    w = img.width,
                    h = img.height,
                    fmt = img.format,
                ));
                html.push('\n');
            }
            html.push_str("</div>\n");
        }

        // Tables
        if !content.tables.is_empty() {
            html.push_str(&format!("<h3>表格 ({})</h3>\n", content.tables.len()));
            for table in &content.tables {
                if let Some(ref caption) = table.caption {
                    html.push_str(&format!(r#"<div class="table-caption">{}</div>"#, html_escape(caption)));
                }
                html.push_str(r#"<table class="data-table"><thead><tr>"#);
                for h in &table.headers {
                    html.push_str(&format!("<th>{}</th>", html_escape(h)));
                }
                html.push_str("</tr></thead><tbody>");
                for row in table.rows.iter().take(20) {
                    html.push_str("<tr>");
                    for cell in row {
                        html.push_str(&format!("<td>{}</td>", html_escape(cell)));
                    }
                    html.push_str("</tr>");
                }
                html.push_str("</tbody></table>\n");
            }
        }

        // No content fallback
        if content.sections.is_empty() && content.formulas.is_empty()
            && content.images.is_empty() && content.tables.is_empty() {
            html.push_str(r#"<div class="empty">未提取到内容</div>"#);
        }

        html.push_str("</div>\n"); // close .paper
    }

    html.push_str("</div>\n</body>\n</html>");
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}
