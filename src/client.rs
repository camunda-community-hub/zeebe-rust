use crate::{
    error::{Error, Result},
    job::{CompleteJobBuilder, FailJobBuilder, ThrowErrorBuilder},
    proto::gateway_client::GatewayClient,
    topology::TopologyBuilder,
    util::{PublishMessageBuilder, ResolveIncidentBuilder},
    worker::JobWorkerBuilder,
    workflow::{
        CancelWorkflowInstanceBuilder, CreateWorkflowInstanceBuilder,
        CreateWorkflowInstanceWithResultBuilder, DeployWorkflowBuilder,
    },
};
use std::fmt::Debug;
use tonic::transport::{Channel, ClientTlsConfig};

/// Client used to communicate with Zeebe.
#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) gateway_client: GatewayClient<Channel>,
}

impl Default for Client {
    fn default() -> Self {
        Client::from_config(ClientConfig::default()).unwrap()
    }
}

impl Client {
    /// Create a new client with default config.
    pub fn new() -> Self {
        Client::default()
    }

    /// Build a new Zeebe client from a given configuration.
    pub fn from_config(config: ClientConfig) -> Result<Self> {
        let channel = Self::build_channel(config)?;

        Ok(Client {
            gateway_client: GatewayClient::new(channel),
        })
    }

    /// Obtains the current topology of the cluster the gateway is part of.
    pub fn topology(&mut self) -> TopologyBuilder<'_> {
        TopologyBuilder::new(self)
    }

    /// Create a new deploy workflow builder.
    pub fn deploy_workflow<T: Into<String>>(&mut self, workflow: T) -> DeployWorkflowBuilder<'_> {
        DeployWorkflowBuilder::new(self, workflow)
    }

    /// Create a new workflow instance builder.
    pub fn create_workflow_instance(&mut self) -> CreateWorkflowInstanceBuilder<'_> {
        CreateWorkflowInstanceBuilder::new(self)
    }

    /// Create a new workflow instance with result builder.
    pub fn create_workflow_instance_with_result(
        &mut self,
    ) -> CreateWorkflowInstanceWithResultBuilder<'_> {
        CreateWorkflowInstanceWithResultBuilder::new(self)
    }

    /// Create a new cancel workflow instance builder.
    pub fn cancel_workflow_instance(&mut self) -> CancelWorkflowInstanceBuilder<'_> {
        CancelWorkflowInstanceBuilder::new(self)
    }

    /// Create a new job worker builder.
    pub fn job_worker(&mut self) -> JobWorkerBuilder<'_> {
        JobWorkerBuilder::new(self)
    }

    /// Completes a job with the given payload, which allows completing the
    /// associated service task.
    pub fn complete_job(&mut self) -> CompleteJobBuilder {
        CompleteJobBuilder::new(self.clone())
    }

    /// Marks the job as failed.
    ///
    /// If the retries argument is positive, then the job will be immediately
    /// activatable again, and a worker could try again to process it. If it is zero
    /// or negative however, an incident will be raised, tagged with the given
    /// error_message, and the job will not be activatable until the incident is
    /// resolved.
    pub fn fail_job(&mut self) -> FailJobBuilder {
        FailJobBuilder::new(self.clone())
    }

    /// Throw an error to indicate that a business error has occurred while
    /// processing the job.
    ///
    /// The error is identified by an error code and is handled by an error catch
    /// event in the workflow with the same error code.
    pub fn throw_error(&mut self) -> ThrowErrorBuilder<'_> {
        ThrowErrorBuilder::new(self)
    }

    /// Publishes a single message. Messages are published to specific partitions
    /// computed from their correlation keys.
    pub fn publish_message(&mut self) -> PublishMessageBuilder<'_> {
        PublishMessageBuilder::new(self)
    }

    /// Resolves a given incident.
    ///
    /// This simply marks the incident as resolved; most likely a call to
    /// [`update_job_retries`] or [`update_workload_instance_payload`] will be
    /// necessary to actually resolve the problem, following by this call.
    ///
    /// [`update_job_retries`]: struct.Client.html#fn.update_job_retries
    /// [`update_workload_instance_payload`]: struct.Client.html#fn.update_workload_instance_payload
    pub fn resolve_incident(&mut self) -> ResolveIncidentBuilder<'_> {
        ResolveIncidentBuilder::new(self)
    }

    fn build_channel(config: ClientConfig) -> Result<Channel> {
        let ClientConfig { endpoints, tls, .. } = config;
        let endpoints = endpoints
            .into_iter()
            .map(|uri| {
                Channel::from_shared(uri.clone()).map_err(|err| Error::InvalidGatewayUri {
                    uri,
                    message: err.to_string(),
                })
            })
            .map(|c| {
                c.and_then(|c| match &tls {
                    Some(tls) => c.tls_config(tls.to_owned()).map_err(From::from),
                    None => Ok(c),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Channel::balance_list(endpoints.into_iter()))
    }
}

/// Config for establishing zeebe client.
#[derive(Debug)]
pub struct ClientConfig {
    /// The endpoints the client should connect to
    pub endpoints: Vec<String>,
    /// TLS configuration
    pub tls: Option<ClientTlsConfig>,
}

impl ClientConfig {
    /// Set the grpc endpoints the client should connect to.
    pub fn with_endpoints(endpoints: Vec<String>) -> Self {
        ClientConfig {
            endpoints,
            ..Default::default()
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            endpoints: vec!["http://0.0.0.0:26500".to_string()],
            tls: None,
        }
    }
}
