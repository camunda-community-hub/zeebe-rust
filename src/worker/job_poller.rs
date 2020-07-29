use crate::{client::Client, proto};
use futures::{
    future::TryFutureExt,
    stream::{BoxStream, StreamExt},
};
use std::fmt;
use std::future::Future;
use std::time::Duration;
use tokio::{sync::mpsc, time::timeout};
use tonic::codegen::{Context, Pin, Poll};

pub(crate) struct JobPoller {
    pub(crate) client: Client,
    pub(crate) request: proto::ActivateJobsRequest,
    pub(crate) request_timeout: Duration,
    pub(crate) request_in_progress: bool,
    pub(crate) max_jobs_active: u32,
    pub(crate) job_queue: mpsc::Sender<Job>,
    pub(crate) message_sender: mpsc::Sender<PollMessage>,
    pub(crate) messages: BoxStream<'static, PollMessage>,
    pub(crate) remaining: u32,
    pub(crate) threshold: u32,
}

impl fmt::Debug for JobPoller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JobPoller")
    }
}

#[derive(Debug)]
pub(crate) enum PollMessage {
    FetchJobs,
    JobsArrived(u32),
    JobFetchFailed,
    JobFinished,
}

/// An activate Zeebe job that is ready to be worked on by a worker.
#[derive(Debug)]
pub struct Job(proto::ActivatedJob);

impl Job {
    /// the key, a unique identifier for the job
    pub fn key(&self) -> i64 {
        self.0.key
    }

    /// the type of the job (should match what was requested)
    pub fn job_type(&self) -> &str {
        &self.0.r#type
    }

    /// the job's workflow instance key
    pub fn workflow_instance_key(&self) -> i64 {
        self.0.workflow_instance_key
    }

    /// the bpmn process ID of the job workflow definition
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the version of the job workflow definition
    pub fn workflow_definition_version(&self) -> i32 {
        self.0.workflow_definition_version
    }

    /// the key of the job workflow definition
    pub fn workflow_key(&self) -> i64 {
        self.0.workflow_key
    }

    /// the associated task element ID
    pub fn element_id(&self) -> &str {
        &self.0.element_id
    }

    /// the unique key identifying the associated task, unique within the scope of
    /// the workflow instance
    pub fn element_instance_key(&self) -> i64 {
        self.0.element_instance_key
    }

    /// a set of custom headers defined during modelling; returned as a serialized
    /// JSON document
    pub fn custom_headers(&self) -> &str {
        &self.0.custom_headers
    }

    /// the name of the worker which activated this job
    pub fn worker(&self) -> &str {
        &self.0.worker
    }

    /// the amount of retries left to this job (should always be positive)
    pub fn retries(&self) -> i32 {
        self.0.retries
    }

    /// when the job can be activated again, sent as a UNIX epoch timestamp
    pub fn deadline(&self) -> i64 {
        self.0.deadline
    }

    /// JSON document, computed at activation time, consisting of all visible
    /// variables to the task scope
    pub fn variables(&self) -> &str {
        &self.0.variables
    }
}

impl JobPoller {
    fn should_activate_jobs(&self) -> bool {
        self.remaining <= self.threshold && !self.request_in_progress
    }
    fn activate_jobs(&mut self) {
        self.request.max_jobs_to_activate = (self.max_jobs_active - self.remaining) as i32;
        let mut job_queue = self.job_queue.clone();
        let mut poll_queue = self.message_sender.clone();
        let worker = self.request.worker.clone();
        let mut gateway_client = self.client.gateway_client.clone();
        let request = self.request.clone();

        tokio::spawn(timeout(self.request_timeout, async move {
            tracing::trace!(?worker, "fetching new jobs");

            if let Ok(mut stream) = gateway_client
                .activate_jobs(tonic::Request::new(request))
                .map_ok(|response| response.into_inner())
                .map_err(|err| {
                    tracing::error!(?worker, ?err, "Failed to fetch new jobs");
                })
                .await
            {
                let mut total_jobs = 0;
                while let Some(Ok(batch)) = stream.next().await {
                    total_jobs += batch.jobs.len() as u32;
                    for job in batch.jobs {
                        let _ = job_queue
                            .send(Job(job))
                            .inspect_err(|err| {
                                tracing::error!(?worker, ?err, "job queue send failed");
                            })
                            .await;
                    }
                }
                tracing::trace!(?worker, "received {} new job(s)", total_jobs);
                let _ = poll_queue
                    .send(PollMessage::JobsArrived(total_jobs))
                    .inspect_err(|err| {
                        tracing::error!(?worker, ?err, "poll queue send failed");
                    })
                    .await;
            } else {
                let _ = poll_queue
                    .send(PollMessage::JobFetchFailed)
                    .inspect_err(|err| {
                        tracing::error!(?worker, ?err, "fetch failed poll queue send failed");
                    })
                    .await;
            }
        }));
    }
}

impl Future for JobPoller {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match futures::ready!(self.messages.poll_next_unpin(cx)) {
                // new work arrived
                Some(PollMessage::JobsArrived(new_job_count)) => {
                    self.remaining = self.remaining.saturating_add(new_job_count);
                    self.request_in_progress = false;
                }
                // fetching new work failed
                Some(PollMessage::JobFetchFailed) => {
                    self.request_in_progress = false;
                }
                // a job was finished by a worker
                Some(PollMessage::JobFinished) => {
                    self.remaining = self.remaining.saturating_sub(1);
                }
                // the poll interval elapsed and we are ready for more work
                Some(PollMessage::FetchJobs) if self.should_activate_jobs() => {
                    self.request_in_progress = true;
                    self.activate_jobs();
                }
                // ignore poll interval if not ready for more work
                Some(PollMessage::FetchJobs) => {}
                // poller should stop
                None => return Poll::Ready(()),
            }
        }
    }
}
