use tokio_cron_scheduler::{Job, JobScheduler};
use anyhow::Result;
use tracing::info;
use std::sync::Arc;

pub struct TaskScheduler {
    scheduler: JobScheduler,
}

impl TaskScheduler {
    pub async fn new() -> Result<Self> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self { scheduler })
    }

    pub async fn add_daily_job<F>(&self, cron_expr: &str, job_fn: Arc<F>) -> Result<()>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let job = Job::new_async(cron_expr, move |_uuid, _lock| {
            let job_fn = Arc::clone(&job_fn);
            Box::pin(async move {
                info!("执行定时任务");
                job_fn();
            })
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        self.scheduler.start().await?;
        info!("任务调度器已启动");
        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        self.scheduler.shutdown().await?;
        info!("任务调度器已关闭");
        Ok(())
    }
}
