use crate::client::Client;
use crate::error::{Error, Result};
use crate::proto;
use crate::worker::{job_dispatcher, Job, JobPoller, PollMessage};
use futures::future::BoxFuture;
use futures::StreamExt;
use serde::export::Formatter;
use std::fmt;
use std::future::Future;
use std::time::Duration;
use tokio::{sync::mpsc, time::interval};

static DEFAULT_JOB_TIMEOUT: Duration = Duration::from_secs(5 * 60);
static DEFAULT_JOB_TIMEOUT_IN_MS: i64 = DEFAULT_JOB_TIMEOUT.as_millis() as i64;

static DEFAULT_JOB_WORKER_MAX_JOB_ACTIVE: u32 = 32;
static DEFAULT_JOB_WORKER_CONCURRENCY: u32 = 4;
static DEFAULT_JOB_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(100);
static DEFAULT_JOB_WORKER_POLL_THRESHOLD: f32 = 0.3;
static REQUEST_TIMEOUT_OFFSET: Duration = Duration::from_secs(10);
static DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct JobHandler(Box<dyn Fn(Client, Job) -> BoxFuture<'static, Result<()>>>);

impl JobHandler {
    pub(crate) fn call(&self, client: Client, job: Job) -> BoxFuture<'static, Result<()>> {
        self.0(client, job)
    }
}

impl fmt::Debug for JobHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "JobHandler")
    }
}

/// Configuration for an asynchronous worker process.
#[derive(Debug)]
pub struct JobWorkerBuilder<'a> {
    client: &'a mut Client,
    handler: Option<JobHandler>,
    concurrency: u32,
    poll_interval: Duration,
    poll_threshold: f32,
    request: proto::ActivateJobsRequest,
}

impl<'a> JobWorkerBuilder<'a> {
    /// Create a new job worker builder.
    pub fn new(client: &'a mut Client) -> Self {
        JobWorkerBuilder {
            client,
            handler: None,
            concurrency: DEFAULT_JOB_WORKER_CONCURRENCY,
            poll_interval: DEFAULT_JOB_WORKER_POLL_INTERVAL,
            poll_threshold: DEFAULT_JOB_WORKER_POLL_THRESHOLD,
            request: proto::ActivateJobsRequest {
                r#type: String::new(),
                worker: String::from("default"),
                timeout: DEFAULT_JOB_TIMEOUT_IN_MS,
                max_jobs_to_activate: DEFAULT_JOB_WORKER_MAX_JOB_ACTIVE as i32,
                fetch_variable: Vec::new(),
                request_timeout: (DEFAULT_REQUEST_TIMEOUT + REQUEST_TIMEOUT_OFFSET).as_millis()
                    as i64,
            },
        }
    }

    /// Set the job type of the worker.
    pub fn with_job_type<T: Into<String>>(mut self, job_type: T) -> Self {
        self.request.r#type = job_type.into();
        self
    }

    /// Set the worker name (mostly used for logging)
    pub fn with_worker_name<T: Into<String>>(mut self, worker: T) -> Self {
        self.request.worker = worker.into();
        self
    }

    /// Set the handler function for the worker.
    pub fn with_handler<T, R>(self, handler: T) -> Self
    where
        T: Fn(Client, Job) -> R + 'static,
        R: Future<Output = Result<()>> + Send + 'static,
    {
        JobWorkerBuilder {
            handler: Some(JobHandler(Box::new(move |client, job| {
                Box::pin(handler(client, job))
            }))),
            ..self
        }
    }

    /// Start the worker as a future. To stop the worker, simply drop the future.
    pub async fn spawn(self) -> Result<()> {
        if self.request.r#type.is_empty() || self.handler.is_none() {
            return Err(Error::InvalidParameters(
                "`job_type` and `handler` must be set",
            ));
        }

        let (job_queue, job_queue_rx) = mpsc::channel(self.request.max_jobs_to_activate as usize);
        let (poll_queue, poll_rx) = mpsc::channel(32);
        let poll_interval = interval(self.poll_interval).map(|_| PollMessage::FetchJobs);
        let worker_name = self.request.worker.clone();
        let job_poller = JobPoller {
            client: self.client.clone(),
            request_timeout: Duration::from_millis(self.request.request_timeout as u64),
            request_in_progress: false,
            max_jobs_active: self.request.max_jobs_to_activate as u32,
            job_queue,
            message_sender: poll_queue.clone(),
            messages: Box::pin(futures::stream::select(poll_rx, poll_interval)),
            remaining: 0,
            threshold: (self.request.max_jobs_to_activate as f32 * self.poll_threshold).floor()
                as u32,
            request: self.request,
        };

        // Process work
        futures::join!(
            job_poller,
            job_dispatcher::run(
                job_queue_rx,
                poll_queue,
                self.concurrency as usize,
                self.handler.unwrap(),
                self.client.clone(),
                worker_name,
            )
        );

        Ok(())
    }
}
