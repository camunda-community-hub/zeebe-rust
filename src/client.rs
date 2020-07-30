use crate::{
    error::{Error, Result},
    job::{CompleteJobBuilder, FailJobBuilder, ThrowErrorBuilder, UpdateJobRetriesBuilder},
    proto::gateway_client::GatewayClient,
    topology::TopologyBuilder,
    util::{PublishMessageBuilder, ResolveIncidentBuilder},
    worker::JobWorkerBuilder,
    workflow::{
        CancelWorkflowInstanceBuilder, CreateWorkflowInstanceBuilder,
        CreateWorkflowInstanceWithResultBuilder, DeployWorkflowBuilder, SetVariablesBuilder,
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

    /// Deploys one or more workflows to Zeebe. Note that this is an atomic call,
    /// i.e. either all workflows are deployed, or none of them are.
    pub fn deploy_workflow(&mut self) -> DeployWorkflowBuilder<'_> {
        DeployWorkflowBuilder::new(self)
    }

    /// Creates and starts an instance of the specified workflow.
    ///
    /// The workflow definition to use to create the instance can be specified
    /// either using its unique key (as returned by [`deploy_workflow`]), or using the
    /// BPMN process ID and a version. Pass -1 as the version to use the latest
    /// deployed version.
    ///
    /// Note that only workflows with none start events can be started through this
    /// command.
    ///
    /// [`deploy_workflow`]: struct.Client.html#fn.deploy_workflow
    pub fn create_workflow_instance(&mut self) -> CreateWorkflowInstanceBuilder<'_> {
        CreateWorkflowInstanceBuilder::new(self)
    }

    /// Similar to [`create_workflow_instance`], creates and starts an instance of
    /// the specified workflow.
    ///
    /// Unlike [`create_workflow_instance`], the response is returned when the
    /// workflow is completed.
    ///
    /// Note that only workflows with none start events can be started through this
    /// command.
    ///
    /// [`create_workflow_instance`]: struct.Client.html#fn.create_workflow_instance
    pub fn create_workflow_instance_with_result(
        &mut self,
    ) -> CreateWorkflowInstanceWithResultBuilder<'_> {
        CreateWorkflowInstanceWithResultBuilder::new(self)
    }

    /// Cancels a running workflow instance.
    pub fn cancel_workflow_instance(&mut self) -> CancelWorkflowInstanceBuilder<'_> {
        CancelWorkflowInstanceBuilder::new(self)
    }

    /// Updates all the variables of a particular scope (e.g. workflow instance,
    /// flow element instance) from the given JSON document.
    pub fn set_variables(&mut self) -> SetVariablesBuilder<'_> {
        SetVariablesBuilder::new(self)
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
    /// If the `retries` argument is positive, then the job will be immediately
    /// activatable again, and a worker could try again to process it. If it is zero
    /// or negative however, an incident will be raised, tagged with the given
    /// `error_message`, and the job will not be activatable until the incident is
    /// resolved.
    pub fn fail_job(&mut self) -> FailJobBuilder {
        FailJobBuilder::new(self.clone())
    }

    /// Updates the number of retries a job has left.
    ///
    /// This is mostly useful for jobs that have run out of retries, should the
    /// underlying problem be solved.
    pub fn update_job_retries(&mut self) -> UpdateJobRetriesBuilder<'_> {
        UpdateJobRetriesBuilder::new(self)
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
    /// necessary to actually resolve the problem, followed by this call.
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
