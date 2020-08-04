use crate::{
    client::Client,
    job::Job,
    worker::{builder::JobHandler, PollMessage},
};
use futures::StreamExt;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, Semaphore},
    task::{spawn_local, LocalSet},
};

pub(crate) async fn run(
    mut job_queue: mpsc::Receiver<Job>,
    poll_queue: mpsc::Sender<PollMessage>,
    concurrency: usize,
    handler: JobHandler,
    job_client: Client,
    worker: String,
) {
    let concurrent_jobs = Arc::new(Semaphore::new(concurrency));

    while let Some(job) = job_queue.next().await {
        let job_slot = concurrent_jobs.clone().acquire_owned().await;
        let mut task = JobTask {
            job,
            job_client: job_client.clone(),
            poll_queue: poll_queue.clone(),
            handler: handler.clone(),
            worker: worker.clone(),
        };

        let _ = LocalSet::new()
            .run_until(async {
                spawn_local(async move {
                    let key = task.job.key();
                    tracing::trace!(worker = ?task.worker, ?key, job = ?task.job, "dispatching job");
                    task.handler.call(task.job_client, task.job).await;

                    tracing::trace!(worker = ?task.worker, ?key, "job completed");
                    let _ = task.poll_queue.send(PollMessage::JobFinished).await;
                    job_slot
                }).await
            })
            .await;
    }
}

struct JobTask {
    job: Job,
    job_client: Client,
    poll_queue: mpsc::Sender<PollMessage>,
    handler: JobHandler,
    worker: String,
}
