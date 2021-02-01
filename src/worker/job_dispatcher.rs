use crate::{
    client::Client,
    job::Job,
    worker::{auto_handler::Extensions, builder::JobHandler, PollMessage},
};
use std::rc::Rc;
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
    job_extensions: Extensions,
) {
    let concurrent_jobs = Arc::new(Semaphore::new(concurrency));
    let per_job_extensions = Rc::new(job_extensions);

    while let Some(job) = job_queue.recv().await {
        let job_slot = concurrent_jobs.clone().acquire_owned().await;
        let mut task = JobTask {
            job,
            job_client: job_client.clone(),
            poll_queue: poll_queue.clone(),
            handler: handler.clone(),
            worker: worker.clone(),
            extensions: per_job_extensions.clone(),
        };

        let _ = LocalSet::new()
            .run_until(async {
                spawn_local(async move {
                    let key = task.job.key();
                    task.job_client.current_job_key = Some(key);
                    task.job_client.current_job_extensions = Some(task.extensions.clone());

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
    extensions: Rc<Extensions>,
}
