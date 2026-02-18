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

                // 下载PDF
                let pdf_filename = format!("data/papers/{}.pdf", arxiv_id.replace("/", "_"));
                match crawler.download_pdf(&paper.pdf_url, &pdf_filename).await {
                    Ok(_) => {
                        // 更新PDF路径
                        db.update_pdf_path("arxiv", &arxiv_id, &pdf_filename).await?;

                        // 解析PDF前两行
                        let parser = parser::PdfParser::new();
                        match parser.extract_first_lines(&pdf_filename, 2) {
                            Ok(lines) => {
                                info!("PDF前两行内容:");
                                for (i, line) in lines.iter().enumerate() {
                                    info!("  第{}行: {}", i + 1, line);
                                }
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

    // TODO: 实现报告生成逻辑

    info!("✅ 报告生成完成");
    Ok(())
}
