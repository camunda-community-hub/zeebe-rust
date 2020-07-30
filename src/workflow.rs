use crate::{proto, Client, Error, Result};
use tokio::{fs::File, io::AsyncReadExt};
use tracing::debug;

/// Deploys one or more workflows to Zeebe.
///
/// Note that this is an atomic call, i.e. either all workflows are deployed, or
/// none of them are.
#[derive(Debug)]
pub struct DeployWorkflowBuilder<'a> {
    client: &'a mut Client,
    resource_file: String,
    resource_type: WorkflowResourceType,
}

/// The format of the uploaded workflows.
#[derive(Debug)]
pub enum WorkflowResourceType {
    /// FILE type means the gateway will try to detect the resource type using the
    /// file extension of the name field
    File = 0,
    /// extension 'bpmn'
    Bpmn = 1,
    /// extension 'yaml'
    Yaml = 2,
}

impl<'a> DeployWorkflowBuilder<'a> {
    /// Create a new deploy workflow builder.
    pub fn new<T: Into<String>>(client: &'a mut Client, resource_file: T) -> Self {
        DeployWorkflowBuilder {
            client,
            resource_file: resource_file.into(),
            resource_type: WorkflowResourceType::File,
        }
    }

    /// Set the resource type for these workflows.
    pub fn with_resource_type(self, resource_type: WorkflowResourceType) -> Self {
        DeployWorkflowBuilder {
            resource_type,
            ..self
        }
    }

    /// Submit the workflows to the Zeebe brokers.
    #[tracing::instrument(skip(self), fields(method = "deploy_workflow"))]
    pub async fn send(self) -> Result<DeployWorkflowResponse> {
        // Read workflow definition
        let mut file = File::open(&self.resource_file)
            .await
            .map_err(|e| Error::FileIo {
                resource_file: self.resource_file.clone(),
                source: e,
            })?;
        let mut definition = vec![];
        file.read_to_end(&mut definition)
            .await
            .map_err(|e| Error::FileIo {
                resource_file: self.resource_file.clone(),
                source: e,
            })?;

        debug!(file = ?self.resource_file, resource_type = ?self.resource_type, "sending request");

        let res = self
            .client
            .gateway_client
            .deploy_workflow(tonic::Request::new(proto::DeployWorkflowRequest {
                workflows: vec![proto::WorkflowRequestObject {
                    name: self.resource_file,
                    r#type: self.resource_type as i32,
                    definition,
                }],
            }))
            .await?;
        Ok(DeployWorkflowResponse(res.into_inner()))
    }
}

/// Deployed workflow data.
#[derive(Debug)]
pub struct DeployWorkflowResponse(proto::DeployWorkflowResponse);

impl DeployWorkflowResponse {
    /// the unique key identifying the deployment
    pub fn key(&self) -> i64 {
        self.0.key
    }

    /// a list of deployed workflows
    pub fn workflows(&self) -> Vec<WorkflowMetadata> {
        self.0
            .workflows
            .iter()
            .map(|proto| WorkflowMetadata(proto.clone()))
            .collect()
    }
}

/// Metadata information about a workflow.
#[derive(Debug)]
pub struct WorkflowMetadata(proto::WorkflowMetadata);

impl WorkflowMetadata {
    /// the bpmn process ID, as parsed during deployment; together with the version
    /// forms a unique identifier for a specific workflow definition
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the assigned process version
    pub fn version(&self) -> i32 {
        self.0.version
    }

    /// the assigned key, which acts as a unique identifier for this workflow
    pub fn workflow_key(&self) -> i64 {
        self.0.workflow_key
    }

    /// the resource name (see: WorkflowRequestObject.name) from which this workflow
    /// was parsed
    pub fn resource_name(&self) -> &str {
        &self.0.resource_name
    }
}

/// Creates and starts an instance of the specified workflow.
///
/// The workflow definition to use to create the instance can be specified
/// either using its unique key (as returned by [DeployWorkflowResponse]), or using the
/// BPMN process ID and a version. Pass -1 as the version to use the latest
/// deployed version.
///
/// Note that only workflows with none start events can be started through this
/// command.
///
/// [DeployWorkflowResponse]: struct.DeployWorkflowResponse.html
#[derive(Debug)]
pub struct CreateWorkflowInstanceBuilder<'a> {
    client: &'a mut Client,
    /// the unique key identifying the workflow definition (e.g. returned from a
    /// workflow in the DeployWorkflowResponse message)
    workflow_key: Option<i64>,
    /// the BPMN process ID of the workflow definition
    bpmn_process_id: Option<String>,
    /// the version of the process; set to -1 to use the latest version
    version: i32,
    /// JSON document that will instantiate the variables for the root variable
    /// scope of the workflow instance; it must be a JSON object, as variables will
    /// be mapped in a key-value fashion. e.g. { "a": 1, "b": 2 } will create two
    /// variables, named "a" and "b" respectively, with their associated values. [{
    /// "a": 1, "b": 2 }] would not be a valid argument, as the root of the JSON
    /// document is an array and not an object.
    variables: Option<serde_json::Value>,
}

impl<'a> CreateWorkflowInstanceBuilder<'a> {
    /// Create a new workflow instance builder
    pub fn new(client: &'a mut Client) -> Self {
        CreateWorkflowInstanceBuilder {
            client,
            workflow_key: None,
            bpmn_process_id: None,
            version: -1,
            variables: None,
        }
    }

    /// Set the workflow key for this workflow instance.
    pub fn with_workflow_key(self, workflow_key: i64) -> Self {
        CreateWorkflowInstanceBuilder {
            workflow_key: Some(workflow_key),
            ..self
        }
    }

    /// Set the BPMN process id for this workflow instance.
    pub fn with_bpmn_process_id<T: Into<String>>(self, bpmn_process_id: T) -> Self {
        CreateWorkflowInstanceBuilder {
            bpmn_process_id: Some(bpmn_process_id.into()),
            ..self
        }
    }

    /// Set the version for this workflow instance.
    pub fn with_version(self, version: i32) -> Self {
        CreateWorkflowInstanceBuilder { version, ..self }
    }

    /// Use the latest workflow version for this workflow instance.
    pub fn with_latest_version(self) -> Self {
        CreateWorkflowInstanceBuilder {
            version: -1,
            ..self
        }
    }

    /// Set variables for this workflow instance.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        CreateWorkflowInstanceBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Submit this workflow instance to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), fields(method = "create_workflow_instance"))]
    pub async fn send(self) -> Result<CreateWorkflowInstanceResponse> {
        if self.workflow_key.is_none() && self.bpmn_process_id.is_none() {
            return Err(Error::InvalidParameters(
                "`workflow_key` or `pbmn_process_id` must be set",
            ));
        }
        let req = proto::CreateWorkflowInstanceRequest {
            workflow_key: self.workflow_key.unwrap_or(0),
            bpmn_process_id: self.bpmn_process_id.unwrap_or_else(String::new),
            version: self.version,
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .create_workflow_instance(tonic::Request::new(req))
            .await?;

        Ok(CreateWorkflowInstanceResponse(res.into_inner()))
    }
}

/// Created workflow instance data.
#[derive(Debug)]
pub struct CreateWorkflowInstanceResponse(proto::CreateWorkflowInstanceResponse);

impl CreateWorkflowInstanceResponse {
    /// the key of the workflow definition which was used to create the workflow
    /// instance
    pub fn workflow_key(&self) -> i64 {
        self.0.workflow_key
    }

    /// the BPMN process ID of the workflow definition which was used to create the
    /// workflow instance
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the version of the workflow definition which was used to create the workflow
    /// instance
    pub fn version(&self) -> i32 {
        self.0.version
    }

    /// the unique identifier of the created workflow instance; to be used wherever
    /// a request needs a workflow instance key (e.g. CancelWorkflowInstanceRequest)
    pub fn workflow_instance_key(&self) -> i64 {
        self.0.workflow_instance_key
    }
}

/// Creates and starts an instance of the specified workflow with result.
///
/// Similar to [`CreateWorkflowInstanceBuilder`], creates and starts an instance of
/// the specified workflow. Unlike [`CreateWorkflowInstanceBuilder`], the response is
/// returned when the workflow is completed.
///
/// Note that only workflows with none start events can be started through this
/// command.
///
/// [`CreateWorkflowInstanceBuilder`]: struct.CreateWorkflowInstanceBuilder.html
#[derive(Debug)]
pub struct CreateWorkflowInstanceWithResultBuilder<'a> {
    client: &'a mut Client,
    /// the unique key identifying the workflow definition (e.g. returned from a
    /// workflow in the DeployWorkflowResponse message)
    workflow_key: Option<i64>,
    /// the BPMN process ID of the workflow definition
    bpmn_process_id: Option<String>,
    /// the version of the process; set to -1 to use the latest version
    version: i32,
    /// JSON document that will instantiate the variables for the root variable
    /// scope of the workflow instance; it must be a JSON object, as variables will
    /// be mapped in a key-value fashion. e.g. { "a": 1, "b": 2 } will create two
    /// variables, named "a" and "b" respectively, with their associated values. [{
    /// "a": 1, "b": 2 }] would not be a valid argument, as the root of the JSON
    /// document is an array and not an object.
    variables: Option<serde_json::Value>,
    /// timeout (in ms). the request will be closed if the workflow is not completed before
    /// the requestTimeout.
    ///
    /// if request_timeout = 0, uses the generic requestTimeout configured in the gateway.
    request_timeout: u64,
    /// list of names of variables to be included in
    /// [`CreateWorkflowInstanceWithResultResponse`]'s variables if empty, all visible
    /// variables in the root scope will be returned.
    ///
    /// [`CreateWorkflowInstanceWithResultResponse`]: struct.CreateWorkflowInstanceWithResultResponse.html
    fetch_variables: Vec<String>,
}

impl<'a> CreateWorkflowInstanceWithResultBuilder<'a> {
    /// Create a new workflow instance builder
    pub fn new(client: &'a mut Client) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            client,
            workflow_key: None,
            bpmn_process_id: None,
            version: -1,
            variables: None,
            request_timeout: 0,
            fetch_variables: Vec::new(),
        }
    }

    /// Set the workflow key for this workflow instance.
    pub fn with_workflow_key(self, workflow_key: i64) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            workflow_key: Some(workflow_key),
            ..self
        }
    }

    /// Set the BPMN process id for this workflow instance.
    pub fn with_bpmn_process_id<T: Into<String>>(self, bpmn_process_id: T) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            bpmn_process_id: Some(bpmn_process_id.into()),
            ..self
        }
    }

    /// Set the version for this workflow instance.
    pub fn with_version(self, version: i32) -> Self {
        CreateWorkflowInstanceWithResultBuilder { version, ..self }
    }

    /// Use the latest workflow version for this workflow instance.
    pub fn with_latest_version(self) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            version: -1,
            ..self
        }
    }

    /// Set variables for this workflow instance.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Set variables for this workflow instance.
    pub fn with_fetch_variables(self, fetch_variables: Vec<String>) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            fetch_variables,
            ..self
        }
    }

    /// Set the result timeout for this workflow instance request.
    pub fn with_request_timeout(self, request_timeout: u64) -> Self {
        CreateWorkflowInstanceWithResultBuilder {
            request_timeout,
            ..self
        }
    }

    /// Submit this workflow instance to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), fields(method = "create_workflow_instance_with_result"))]
    pub async fn send(self) -> Result<CreateWorkflowInstanceWithResultResponse> {
        if self.workflow_key.is_none() && self.bpmn_process_id.is_none() {
            return Err(Error::InvalidParameters(
                "`workflow_key` or `pbmn_process_id` must be set",
            ));
        }
        let req = proto::CreateWorkflowInstanceWithResultRequest {
            request: Some(proto::CreateWorkflowInstanceRequest {
                workflow_key: self.workflow_key.unwrap_or(0),
                bpmn_process_id: self.bpmn_process_id.unwrap_or_else(String::new),
                version: self.version,
                variables: self
                    .variables
                    .map_or(String::new(), |vars| vars.to_string()),
            }),
            request_timeout: self.request_timeout as i64,
            fetch_variables: self.fetch_variables,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .create_workflow_instance_with_result(tonic::Request::new(req))
            .await?;

        Ok(CreateWorkflowInstanceWithResultResponse(res.into_inner()))
    }
}

/// Created workflow instance with result data.
#[derive(Debug)]
pub struct CreateWorkflowInstanceWithResultResponse(
    proto::CreateWorkflowInstanceWithResultResponse,
);

impl CreateWorkflowInstanceWithResultResponse {
    /// the key of the workflow definition which was used to create the workflow
    /// instance
    pub fn workflow_key(&self) -> i64 {
        self.0.workflow_key
    }

    /// the BPMN process ID of the workflow definition which was used to create the
    /// workflow instance
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the version of the workflow definition which was used to create the workflow
    /// instance
    pub fn version(&self) -> i32 {
        self.0.version
    }

    /// the unique identifier of the created workflow instance; to be used wherever
    /// a request needs a workflow instance key (e.g. CancelWorkflowInstanceRequest)
    pub fn workflow_instance_key(&self) -> i64 {
        self.0.workflow_instance_key
    }

    /// JSON document consists of visible variables in the root scope
    pub fn variables(&self) -> &str {
        &self.0.variables
    }
}

/// Cancels a running workflow instance.
#[derive(Debug)]
pub struct CancelWorkflowInstanceBuilder<'a> {
    client: &'a mut Client,
    /// the unique key identifying the workflow definition (e.g. returned from a
    /// workflow in the DeployWorkflowResponse message)
    workflow_key: Option<i64>,
}

impl<'a> CancelWorkflowInstanceBuilder<'a> {
    /// Create a new cancel workflow instance builder
    pub fn new(client: &'a mut Client) -> Self {
        CancelWorkflowInstanceBuilder {
            client,
            workflow_key: None,
        }
    }

    /// Set the workflow key for this instance.
    pub fn with_workflow_key(self, workflow_key: i64) -> Self {
        CancelWorkflowInstanceBuilder {
            workflow_key: Some(workflow_key),
            ..self
        }
    }

    /// Submit this workflow instance to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), fields(method = "cancel_workflow_instance"))]
    pub async fn send(self) -> Result<CancelWorkflowInstanceResponse> {
        if self.workflow_key.is_none() {
            return Err(Error::InvalidParameters("`workflow_key` must be set"));
        }
        let req = proto::CancelWorkflowInstanceRequest {
            workflow_instance_key: self.workflow_key.unwrap(),
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .cancel_workflow_instance(tonic::Request::new(req))
            .await?;

        Ok(CancelWorkflowInstanceResponse(res.into_inner()))
    }
}

/// Canceled workflow instance data.
#[derive(Debug)]
pub struct CancelWorkflowInstanceResponse(proto::CancelWorkflowInstanceResponse);

/// Updates all the variables of a particular scope (e.g. workflow instance, flow
/// element instance) from the given JSON document.
#[derive(Debug)]
pub struct SetVariablesBuilder<'a> {
    client: &'a mut Client,
    element_instance_key: Option<i64>,
    variables: Option<serde_json::Value>,
    local: bool,
}

impl<'a> SetVariablesBuilder<'a> {
    /// Create a new set variables builder
    pub fn new(client: &'a mut Client) -> Self {
        SetVariablesBuilder {
            client,
            element_instance_key: None,
            variables: None,
            local: false,
        }
    }

    /// Set the unique identifier of this element.
    ///
    /// can be the workflow instance key (as obtained during instance creation), or
    /// a given element, such as a service task (see `element_instance_key` on the job
    /// message).
    pub fn with_element_instance_key(self, element_instance_key: i64) -> Self {
        SetVariablesBuilder {
            element_instance_key: Some(element_instance_key),
            ..self
        }
    }

    /// Set variables for this element.
    ///
    /// Variables are a JSON serialized document describing variables as key value
    /// pairs; the root of the document must be a JSON object.
    // must be an object
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        SetVariablesBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Set local scope for this request.
    ///
    /// If set to `true`, the variables will be merged strictly into the local scope
    /// (as indicated by element_instance_key); this means the variables are not
    /// propagated to upper scopes.
    ///
    /// ## Example
    ///
    /// Two scopes:
    ///
    /// * 1 => `{ "foo" : 2 }`
    /// * 2 => `{ "bar" : 1 }`
    ///
    /// If we send an update request with `element_instance_key` = `2`, variables
    /// `{ "foo" : 5 }`, and `local` is `true`, then the result is:
    ///
    /// * 1 => `{ "foo" : 2 }`
    /// * 2 => `{ "bar" : 1, "foo" 5 }`
    ///
    /// If `local` was `false`, however, then the result is:
    ///
    /// * 1 => `{ "foo": 5 }`,
    /// * 2 => `{ "bar" : 1 }`
    pub fn with_local(self, local: bool) -> Self {
        SetVariablesBuilder { local, ..self }
    }

    /// Submit this workflow instance to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), fields(method = "set_variables"))]
    pub async fn send(self) -> Result<SetVariablesResponse> {
        if self.element_instance_key.is_none() {
            return Err(Error::InvalidParameters(
                "`element_instance_key` must be set",
            ));
        }
        let req = proto::SetVariablesRequest {
            element_instance_key: self.element_instance_key.unwrap(),
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
            local: self.local,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .set_variables(tonic::Request::new(req))
            .await?;

        Ok(SetVariablesResponse(res.into_inner()))
    }
}

/// Set variables data.
#[derive(Debug)]
pub struct SetVariablesResponse(proto::SetVariablesResponse);

impl SetVariablesResponse {
    /// The unique key of the set variables command.
    pub fn key(&self) -> i64 {
        self.0.key
    }
}
