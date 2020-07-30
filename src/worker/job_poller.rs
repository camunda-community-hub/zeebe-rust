use crate::{client::Client, job::Job, proto};
use futures::{
    future::{FutureExt, TryFutureExt},
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
    FetchJobsComplete,
    JobFinished,
}

impl JobPoller {
    fn should_activate_jobs(&self) -> bool {
        self.remaining <= self.threshold && !self.request_in_progress
    }

    fn activate_jobs(&mut self) {
        self.request.max_jobs_to_activate = (self.max_jobs_active - self.remaining) as i32;
        let mut job_queue = self.job_queue.clone();
        let mut poll_queue = self.message_sender.clone();
        let mut retry_queue = self.message_sender.clone();
        let worker = self.request.worker.clone();
        let retry_worker = self.request.worker.clone();
        let mut gateway_client = self.client.gateway_client.clone();
        let request = self.request.clone();

        tokio::spawn(
            timeout(self.request_timeout, async move {
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
                                .send(Job::new(job))
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
                }
            })
            .then(|_| async move {
                let _ = retry_queue
                    .send(PollMessage::FetchJobsComplete)
                    .inspect_err(|err| {
                        tracing::error!(?retry_worker, ?err, "fetch failed poll queue send failed");
                    })
                    .await;
            }),
        );
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
                // fetching jobs either completed or failed, either way ready to fetch again.
                Some(PollMessage::FetchJobsComplete) => {
                    self.request_in_progress = false;
                }
                // poller should stop
                None => return Poll::Ready(()),
            }
        }
    }
}
