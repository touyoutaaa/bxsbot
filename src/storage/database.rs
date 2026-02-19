use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use anyhow::Result;
use tracing::info;
use crate::storage::models::Paper;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        // 确保使用create_if_missing选项
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(
                database_url.parse::<sqlx::sqlite::SqliteConnectOptions>()?
                    .create_if_missing(true)
            )
            .await?;

        info!("数据库连接成功: {}", database_url);
        Ok(Self { pool })
    }

    pub async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS papers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                title_zh TEXT,
                authors TEXT,
                abstract TEXT,
                abstract_zh TEXT,
                publish_date TEXT,
                source TEXT NOT NULL,
                source_id TEXT NOT NULL,
                pdf_url TEXT,
                pdf_path TEXT,
                processed INTEGER DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(source, source_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                keywords TEXT NOT NULL,
                sources TEXT NOT NULL,
                categories TEXT,
                enabled INTEGER DEFAULT 1,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS extracted_content (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                paper_id INTEGER NOT NULL,
                formulas TEXT,
                images TEXT,
                tables TEXT,
                key_points TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (paper_id) REFERENCES papers(id),
                UNIQUE(paper_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS reports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                subscription_id INTEGER,
                report_date TEXT NOT NULL,
                paper_count INTEGER,
                ppt_path TEXT,
                status TEXT DEFAULT 'pending',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (subscription_id) REFERENCES subscriptions(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        info!("数据库表结构初始化完成");
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 保存论文到数据库
    pub async fn save_paper(&self, paper: &Paper) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO papers (title, authors, abstract, publish_date, source, source_id, pdf_url, pdf_path)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(source, source_id) DO UPDATE SET
                title = excluded.title,
                authors = excluded.authors,
                abstract = excluded.abstract,
                pdf_url = excluded.pdf_url,
                pdf_path = excluded.pdf_path
            "#,
        )
        .bind(&paper.title)
        .bind(&paper.authors)
        .bind(&paper.abstract_text)
        .bind(&paper.publish_date)
        .bind(&paper.source)
        .bind(&paper.source_id)
        .bind(&paper.pdf_url)
        .bind(&paper.pdf_path)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// 检查论文是否已存在
    pub async fn paper_exists(&self, source: &str, source_id: &str) -> Result<bool> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM papers WHERE source = ? AND source_id = ?"
        )
        .bind(source)
        .bind(source_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result > 0)
    }

    /// 更新论文的PDF路径
    pub async fn update_pdf_path(&self, source: &str, source_id: &str, pdf_path: &str) -> Result<()> {
        sqlx::query(
            "UPDATE papers SET pdf_path = ? WHERE source = ? AND source_id = ?"
        )
        .bind(pdf_path)
        .bind(source)
        .bind(source_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 保存提取内容到 extracted_content 表（upsert）
    pub async fn save_extracted_content(
        &self,
        paper_id: i64,
        formulas: &str,
        images: &str,
        tables: &str,
        key_points: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO extracted_content (paper_id, formulas, images, tables, key_points)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(paper_id) DO UPDATE SET
                formulas = excluded.formulas,
                images = excluded.images,
                tables = excluded.tables,
                key_points = excluded.key_points
            "#,
        )
        .bind(paper_id)
        .bind(formulas)
        .bind(images)
        .bind(tables)
        .bind(key_points)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 标记论文已处理
    pub async fn mark_paper_processed(&self, source: &str, source_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE papers SET processed = 1 WHERE source = ? AND source_id = ?"
        )
        .bind(source)
        .bind(source_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 更新论文的中文翻译
    pub async fn update_translation(
        &self,
        source: &str,
        source_id: &str,
        title_zh: &str,
        abstract_zh: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE papers SET title_zh = ?, abstract_zh = ? WHERE source = ? AND source_id = ?"
        )
        .bind(title_zh)
        .bind(abstract_zh)
        .bind(source)
        .bind(source_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 获取未翻译的论文列表
    pub async fn get_untranslated_papers(&self) -> Result<Vec<Paper>> {
        let papers = sqlx::query_as::<_, Paper>(
            r#"SELECT id, title, title_zh, authors,
                      abstract AS abstract_text, abstract_zh,
                      publish_date, source, source_id,
                      pdf_url, pdf_path, processed, created_at
               FROM papers
               WHERE title_zh IS NULL AND abstract IS NOT NULL"#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(papers)
    }

    /// 清空所有缓存数据表（保留 subscriptions）
    pub async fn clear_all_tables(&self) -> Result<()> {
        // 先删有外键依赖的表
        sqlx::query("DELETE FROM extracted_content").execute(&self.pool).await?;
        sqlx::query("DELETE FROM reports").execute(&self.pool).await?;
        sqlx::query("DELETE FROM papers").execute(&self.pool).await?;
        info!("数据库表已清空");
        Ok(())
    }

    /// 获取所有论文
    pub async fn get_all_papers(&self) -> Result<Vec<Paper>> {
        let papers = sqlx::query_as::<_, Paper>(
            r#"SELECT id, title, title_zh, authors,
                      abstract AS abstract_text, abstract_zh,
                      publish_date, source, source_id,
                      pdf_url, pdf_path, processed, created_at
               FROM papers"#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(papers)
    }
}
