use crate::{
    client::Client,
    worker::{builder::JobHandler, Job, PollMessage},
};
use futures::StreamExt;
use tokio::sync::{mpsc, Semaphore};

pub(crate) async fn run(
    mut job_queue: mpsc::Receiver<Job>,
    mut poll_queue: mpsc::Sender<PollMessage>,
    concurrency: usize,
    handler: JobHandler,
    job_client: Client,
    worker: String,
) {
    let concurrent_jobs = Semaphore::new(concurrency);

    while let Some(job) = job_queue.next().await {
        let _job_slot = concurrent_jobs.acquire().await;

        tracing::trace!(?worker, ?job, "EXECUTING JOB!");
        match handler.call(job_client.clone(), job).await {
            Ok(_) => {
                tracing::debug!(?worker, "FINISHED JOB!");
                let _ = poll_queue.send(PollMessage::JobFinished).await;
            }
            Err(err) => {
                tracing::error!(?worker, ?err, "FAILED JOB!");
            }
        }
    }
}
