use crate::{client::Client, proto, Error, Result};
use tracing::{debug, trace};

/// An activate Zeebe job that is ready to be worked on by a worker.
#[derive(Clone, Debug)]
pub struct Job(proto::ActivatedJob);

impl Job {
    /// Create a new job from a GRPC response
    pub(crate) fn new(proto: proto::ActivatedJob) -> Self {
        Job(proto)
    }

    /// the key, a unique identifier for the job
    pub fn key(&self) -> i64 {
        self.0.key
    }

    /// the type of the job (should match what was requested)
    pub fn job_type(&self) -> &str {
        &self.0.r#type
    }

    /// the job's process instance key
    pub fn process_instance_key(&self) -> i64 {
        self.0.process_instance_key
    }

    /// the bpmn process ID of the job process definition
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the version of the job process definition
    pub fn process_definition_version(&self) -> i32 {
        self.0.process_definition_version
    }

    /// the key of the job process definition
    pub fn process_definition_key(&self) -> i64 {
        self.0.process_definition_key
    }

    /// the associated task element ID
    pub fn element_id(&self) -> &str {
        &self.0.element_id
    }

    /// the unique key identifying the associated task, unique within the scope of
    /// the process instance
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

    /// Serialized JSON document, computed at activation time, consisting of all
    /// visible variables to the task scope
    pub fn variables_str(&self) -> &str {
        &self.0.variables
    }

    /// JSON document, computed at activation time, consisting of all visible
    /// variables to the task scope
    pub fn variables(&self) -> serde_json::Value {
        serde_json::from_str(&self.0.variables).unwrap_or_else(|_| serde_json::json!({}))
    }

    /// Deserialize encoded json variables as a given type
    pub fn variables_as<'a, T: serde::de::Deserialize<'a>>(&'a self) -> Option<T> {
        serde_json::from_str::<'a, T>(&self.0.variables).ok()
    }
}

/// Configuration to complete a job
#[derive(Debug)]
pub struct CompleteJobBuilder {
    client: Client,
    job_key: Option<i64>,
    variables: Option<serde_json::Value>,
}

impl CompleteJobBuilder {
    /// Create a new complete job builder.
    pub fn new(client: Client) -> Self {
        CompleteJobBuilder {
            client,
            job_key: None,
            variables: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`Job::key`].
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
    #[tracing::instrument(skip(self), name = "complete_job", err)]
    pub async fn send(mut self) -> Result<CompleteJobResponse> {
        if self.job_key.is_none() && self.client.current_job_key.is_none() {
            return Err(Error::InvalidParameters("`job_key` must be set"));
        }
        let req = proto::CompleteJobRequest {
            job_key: self.job_key.or(self.client.current_job_key).unwrap(),
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
        };

        debug!(job_key = req.job_key, "completing job:");
        trace!(?req, "request:");
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
pub struct FailJobBuilder {
    client: Client,
    job_key: Option<i64>,
    retries: Option<u32>,
    retry_back_off: Option<i64>,
    variables: Option<serde_json::Value>,
    error_message: Option<String>,
}

impl FailJobBuilder {
    /// Create a new fail job builder.
    pub fn new(client: Client) -> Self {
        FailJobBuilder {
            client,
            job_key: None,
            retries: None,
            retry_back_off: None,
            variables: None,
            error_message: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`Job::key`].
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

    /// Set the amount of time in milliseconds to wait before the job is retried.
    pub fn with_retry_back_off(self, retry_backoff: i64) -> Self {
        FailJobBuilder {
            retry_back_off: Some(retry_backoff),
            ..self
        }
    }

    /// Set the JSON document representing the variables in the current task scope.
    ///
    /// JSON document that will instantiate the variables at the local scope of the
    /// job's associated task; it must be a JSON object, as variables will be mapped in a
    /// key-value fashion. e.g. { "a": 1, "b": 2 } will create two variables, named "a" and
    /// "b" respectively, with their associated values. [{ "a": 1, "b": 2 }] would not be a
    /// valid argument, as the root of the JSON document is an array and not an object.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        FailJobBuilder {
            variables: Some(variables.into()),
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
    #[tracing::instrument(skip(self), name = "fail_job", err)]
    pub async fn send(mut self) -> Result<FailJobResponse> {
        if self.job_key.is_none() && self.client.current_job_key.is_none() {
            return Err(Error::InvalidParameters("`job_key` must be set"));
        }
        let req = proto::FailJobRequest {
            job_key: self.job_key.or(self.client.current_job_key).unwrap(),
            retries: self.retries.unwrap_or_default() as i32,
            retry_back_off: self.retry_back_off.unwrap_or_default(),
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
            error_message: self.error_message.unwrap_or_default(),
        };

        debug!(job_key = req.job_key, "failing job:");
        trace!(?req, "request:");
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
pub struct ThrowErrorBuilder {
    client: Client,
    job_key: Option<i64>,
    error_code: Option<String>,
    error_message: Option<String>,
    variables: Option<serde_json::Value>,
}

impl ThrowErrorBuilder {
    /// Create a new throw error builder.
    pub fn new(client: Client) -> Self {
        ThrowErrorBuilder {
            client,
            job_key: None,
            error_code: None,
            error_message: None,
            variables: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`Job::key`].
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

    /// Set the JSON document representing the variables in the current task scope.
    ///
    /// JSON document that will instantiate the variables at the local scope of the
    /// error catch event that catches the thrown error; it must be a JSON object, as variables will be mapped in a
    /// key-value fashion. e.g. { "a": 1, "b": 2 } will create two variables, named "a" and
    /// "b" respectively, with their associated values. [{ "a": 1, "b": 2 }] would not be a
    /// valid argument, as the root of the JSON document is an array and not an object.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        ThrowErrorBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Submit the throw error request.
    #[tracing::instrument(skip(self), name = "throw_error", err)]
    pub async fn send(mut self) -> Result<ThrowErrorResponse> {
        if self.job_key.is_none() && self.client.current_job_key.is_none() {
            return Err(Error::InvalidParameters("`job_key` must be set"));
        }
        let req = proto::ThrowErrorRequest {
            job_key: self.job_key.or(self.client.current_job_key).unwrap(),
            error_code: self.error_code.unwrap_or_default(),
            error_message: self.error_message.unwrap_or_default(),
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
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

/// Updates the number of retries a job has left. This is mostly useful for jobs
/// that have run out of retries, should the underlying problem be solved.
#[derive(Debug)]
pub struct UpdateJobRetriesBuilder {
    client: Client,
    job_key: Option<i64>,
    retries: Option<u32>,
    operation_reference: Option<u64>,
}

impl UpdateJobRetriesBuilder {
    /// Create a new update retries builder.
    pub fn new(client: Client) -> Self {
        UpdateJobRetriesBuilder {
            client,
            job_key: None,
            retries: None,
            operation_reference: None,
        }
    }

    /// Set the unique job identifier, as obtained from [`Job::key`].
    pub fn with_job_key(self, job_key: i64) -> Self {
        UpdateJobRetriesBuilder {
            job_key: Some(job_key),
            ..self
        }
    }

    /// Set the new amount of retries for the job
    pub fn with_retries(self, retries: u32) -> Self {
        UpdateJobRetriesBuilder {
            retries: Some(retries),
            ..self
        }
    }

    /// Set a reference key chosen by the user and will be part of all records resulted from this operation
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        UpdateJobRetriesBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit the update job retries request.
    #[tracing::instrument(skip(self), name = "update_job_retries", err)]
    pub async fn send(mut self) -> Result<UpdateJobRetriesResponse> {
        if (self.job_key.is_none() && self.client.current_job_key.is_none())
            || self.retries.is_none()
        {
            return Err(Error::InvalidParameters(
                "`job_key` and `retries` must be set",
            ));
        }
        let req = proto::UpdateJobRetriesRequest {
            job_key: self.job_key.or(self.client.current_job_key).unwrap(),
            retries: self.retries.unwrap() as i32,
            operation_reference: self.operation_reference,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .update_job_retries(tonic::Request::new(req))
            .await?;

        Ok(UpdateJobRetriesResponse(res.into_inner()))
    }
}

/// Update job retries data.
#[derive(Debug)]
pub struct UpdateJobRetriesResponse(proto::UpdateJobRetriesResponse);
