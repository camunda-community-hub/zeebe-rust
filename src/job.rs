use crate::{client::Client, proto, Error, Result};
use tracing::debug;

/// Configuration to complete a job
#[derive(Debug)]
pub struct CompleteJobBuilder<'a> {
    client: &'a mut Client,
    job_key: Option<i64>,
    variables: Option<serde_json::Value>,
}

impl<'a> CompleteJobBuilder<'a> {
    /// Create a new complete job builder.
    pub fn new(client: &'a mut Client) -> Self {
        CompleteJobBuilder {
            client,
            job_key: None,
            variables: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`ActivateJobsResponse`].
    ///
    /// [`ActivateJobsResponse`]: struct.ActivateJobsResponse.html
    pub fn with_job_key(self, job_key: i64) -> Self {
        CompleteJobBuilder {
            job_key: Some(job_key),
            ..self
        }
    }

    /// Set the JSON document representing the variables in the current task scope.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        CompleteJobBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Submit the complete job request.
    #[tracing::instrument(skip(self), fields(method = "complete_job"))]
    pub async fn send(self) -> Result<CompleteJobResponse> {
        if self.job_key.is_none() {
            return Err(Error::InvalidParameters("`job_key` must be set"));
        }
        let req = proto::CompleteJobRequest {
            job_key: self.job_key.unwrap(),
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .complete_job(tonic::Request::new(req))
            .await?;

        Ok(CompleteJobResponse(res.into_inner()))
    }
}

/// Completed job instance data.
#[derive(Debug)]
pub struct CompleteJobResponse(proto::CompleteJobResponse);

/// Configuration to fail a job
#[derive(Debug)]
pub struct FailJobBuilder<'a> {
    client: &'a mut Client,
    job_key: Option<i64>,
    retries: Option<u32>,
    error_message: Option<String>,
}

impl<'a> FailJobBuilder<'a> {
    /// Create a new fail job builder.
    pub fn new(client: &'a mut Client) -> Self {
        FailJobBuilder {
            client,
            job_key: None,
            retries: None,
            error_message: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`ActivateJobsResponse`].
    ///
    /// [`ActivateJobsResponse`]: struct.ActivateJobsResponse.html
    pub fn with_job_key(self, job_key: i64) -> Self {
        FailJobBuilder {
            job_key: Some(job_key),
            ..self
        }
    }

    /// Set the amount of retries the job should have left.
    pub fn with_retries(self, retries: u32) -> Self {
        FailJobBuilder {
            retries: Some(retries),
            ..self
        }
    }

    /// Set an optional message describing why the job failed. This is particularly
    /// useful if a job runs out of retries and an incident is raised, as it this
    /// message can help explain why an incident was raised.
    pub fn with_error_message<T: Into<String>>(self, error_message: T) -> Self {
        FailJobBuilder {
            error_message: Some(error_message.into()),
            ..self
        }
    }

    /// Submit the fail job request.
    #[tracing::instrument(skip(self), fields(method = "fail_job"))]
    pub async fn send(self) -> Result<FailJobResponse> {
        if self.job_key.is_none() {
            return Err(Error::InvalidParameters("`job_key` must be set"));
        }
        let req = proto::FailJobRequest {
            job_key: self.job_key.unwrap(),
            retries: self.retries.unwrap_or_default() as i32,
            error_message: self.error_message.unwrap_or_default(),
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .fail_job(tonic::Request::new(req))
            .await?;

        Ok(FailJobResponse(res.into_inner()))
    }
}

/// Failed job instance data.
#[derive(Debug)]
pub struct FailJobResponse(proto::FailJobResponse);

/// Configuration to throw an error in the context of a job.
#[derive(Debug)]
pub struct ThrowErrorBuilder<'a> {
    client: &'a mut Client,
    job_key: Option<i64>,
    error_code: Option<String>,
    error_message: Option<String>,
}

impl<'a> ThrowErrorBuilder<'a> {
    /// Create a new throw error builder.
    pub fn new(client: &'a mut Client) -> Self {
        ThrowErrorBuilder {
            client,
            job_key: None,
            error_code: None,
            error_message: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`ActivateJobsResponse`].
    ///
    /// [`ActivateJobsResponse`]: struct.ActivateJobsResponse.html
    pub fn with_job_key(self, job_key: i64) -> Self {
        ThrowErrorBuilder {
            job_key: Some(job_key),
            ..self
        }
    }

    /// Set the error code that will be matched with an error catch event.
    pub fn with_error_code<T: Into<String>>(self, error_code: T) -> Self {
        ThrowErrorBuilder {
            error_code: Some(error_code.into()),
            ..self
        }
    }

    /// Set an optional message describing why the error was thrown.
    pub fn with_error_message<T: Into<String>>(self, error_message: T) -> Self {
        ThrowErrorBuilder {
            error_message: Some(error_message.into()),
            ..self
        }
    }

    /// Submit the throw error request.
    #[tracing::instrument(skip(self), fields(method = "throw_error"))]
    pub async fn send(self) -> Result<ThrowErrorResponse> {
        if self.job_key.is_none() {
            return Err(Error::InvalidParameters("`job_key` must be set"));
        }
        let req = proto::ThrowErrorRequest {
            job_key: self.job_key.unwrap(),
            error_code: self.error_code.unwrap_or_default(),
            error_message: self.error_message.unwrap_or_default(),
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .throw_error(tonic::Request::new(req))
            .await?;

        Ok(ThrowErrorResponse(res.into_inner()))
    }
}

/// Throw error response data.
#[derive(Debug)]
pub struct ThrowErrorResponse(proto::ThrowErrorResponse);
