use crate::{
    client::Client,
    job::Job,
    worker::{auto_handler::Extensions, builder::JobHandler, PollMessage},
};
use futures::StreamExt;
use std::rc::Rc;
use tokio::sync::mpsc;

pub(crate) async fn run(
    job_queue: mpsc::Receiver<Job>,
    poll_queue: mpsc::Sender<PollMessage>,
    concurrency: usize,
    handler: JobHandler,
    job_client: Client,
    worker: String,
    job_extensions: Extensions,
) {
    let per_job_extensions = Rc::new(job_extensions);

    job_queue
        .for_each_concurrent(concurrency, |job| {
            let mut task = JobTask {
                job,
                job_client: job_client.clone(),
                poll_queue: poll_queue.clone(),
                handler: &handler,
                worker: &worker,
                extensions: &per_job_extensions,
            };

            async move {
                let key = task.job.key();
                task.job_client.current_job_key = Some(key);
                task.job_client.current_job_extensions = Some(task.extensions.clone());

                tracing::trace!(worker = ?task.worker, ?key, job = ?task.job, "dispatching job");
                task.handler.call(task.job_client, task.job).await;

                tracing::trace!(worker = ?task.worker, ?key, "job completed");
                let _ = task.poll_queue.send(PollMessage::JobFinished).await;
            }
        })
        .await
}

struct JobTask<'a> {
    job: Job,
    job_client: Client,
    poll_queue: mpsc::Sender<PollMessage>,
    handler: &'a JobHandler,
    worker: &'a str,
    extensions: &'a Rc<Extensions>,
}
