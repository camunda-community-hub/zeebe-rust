use crate::{proto, Client, Error, Result};
use tokio::{fs::File, io::AsyncReadExt};
use tracing::{debug, trace};

/// Deploys one or more process to Zeebe.
///
/// Note that this is an atomic call, i.e. either all processes are deployed, or
/// none of them are.
#[derive(Debug)]
pub struct DeployProcessBuilder {
    client: Client,
    resource_files: Vec<String>,
}

impl DeployProcessBuilder {
    /// Create a new deploy process builder.
    pub fn new(client: Client) -> Self {
        DeployProcessBuilder {
            client,
            resource_files: Vec::new(),
        }
    }

    /// Set a single resource file to upload.
    pub fn with_resource_file<T: Into<String>>(self, resource_file: T) -> Self {
        DeployProcessBuilder {
            resource_files: vec![resource_file.into()],
            ..self
        }
    }

    /// Set a list of resource files to uploaded.
    pub fn with_resource_files(self, resource_files: Vec<String>) -> Self {
        DeployProcessBuilder {
            resource_files,
            ..self
        }
    }

    /// Submit the process to the Zeebe brokers.
    #[tracing::instrument(skip(self), name = "deploy_process", err)]
    pub async fn send(mut self) -> Result<DeployProcessResponse> {
        // Read process definitions
        trace!(files = ?self.resource_files, "reading files");
        let mut processes = Vec::with_capacity(self.resource_files.len());
        for path in self.resource_files.iter() {
            let mut file = File::open(path).await.map_err(|e| Error::FileIo {
                resource_file: path.clone(),
                source: e,
            })?;
            let mut definition = vec![];
            file.read_to_end(&mut definition)
                .await
                .map_err(|e| Error::FileIo {
                    resource_file: path.clone(),
                    source: e,
                })?;
            processes.push(proto::ProcessRequestObject {
                name: path.clone(),
                definition,
            })
        }

        debug!(files = ?self.resource_files, "sending request");

        let res = self
            .client
            .gateway_client
            .deploy_process(tonic::Request::new(proto::DeployProcessRequest {
                processes,
            }))
            .await?;
        Ok(DeployProcessResponse(res.into_inner()))
    }
}

/// Deployed process data.
#[derive(Debug)]
pub struct DeployProcessResponse(proto::DeployProcessResponse);

impl DeployProcessResponse {
    /// the unique key identifying the deployment
    pub fn key(&self) -> i64 {
        self.0.key
    }

    /// a list of deployed processes
    pub fn processes(&self) -> Vec<ProcessMetadata> {
        self.0
            .processes
            .iter()
            .map(|proto| ProcessMetadata(proto.clone()))
            .collect()
    }
}

/// Metadata information about a process.
#[derive(Debug)]
pub struct ProcessMetadata(proto::ProcessMetadata);

impl ProcessMetadata {
    /// the bpmn process ID, as parsed during deployment; together with the version
    /// forms a unique identifier for a specific process definition
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the assigned process version
    pub fn version(&self) -> i32 {
        self.0.version
    }

    /// the assigned key, which acts as a unique identifier for this process
    pub fn process_definition_key(&self) -> i64 {
        self.0.process_definition_key
    }

    /// the resource name (see: ProcessRequestObject.name) from which this process
    /// was parsed
    pub fn resource_name(&self) -> &str {
        &self.0.resource_name
    }
}

/// Creates and starts an instance of the specified process.
///
/// The process definition to use to create the instance can be specified
/// either using its unique key (as returned by [`DeployProcessResponse`]), or using the
/// BPMN process ID and a version. Pass -1 as the version to use the latest
/// deployed version.
///
/// Note that only processes with no start events can be started through this
/// command.
#[derive(Debug)]
pub struct CreateProcessInstanceBuilder {
    client: Client,
    /// the unique key identifying the process definition (e.g. returned from a
    /// process in the [`DeployProcessResponse`] message)
    process_definition_key: Option<i64>,
    /// the BPMN process ID of the process definition
    bpmn_process_id: Option<String>,
    /// the version of the process; set to -1 to use the latest version
    version: i32,
    /// JSON document that will instantiate the variables for the root variable
    /// scope of the process instance; it must be a JSON object, as variables will
    /// be mapped in a key-value fashion. e.g. { "a": 1, "b": 2 } will create two
    /// variables, named "a" and "b" respectively, with their associated values. [{
    /// "a": 1, "b": 2 }] would not be a valid argument, as the root of the JSON
    /// document is an array and not an object.
    variables: Option<serde_json::Value>,
    /// List of start instructions. If empty (default) the process instance
    /// will start at the start event. If non-empty the process instance will apply start
    /// instructions after it has been created
    start_instructions: Vec<String>,
    /// the tenant id of the process definition
    tenant_id: Option<String>,
    /// a reference key chosen by the user and will be part of all records resulted from this operation
    operation_reference: Option<u64>,
}

impl CreateProcessInstanceBuilder {
    /// Create a new process instance builder
    pub fn new(client: Client) -> Self {
        CreateProcessInstanceBuilder {
            client,
            process_definition_key: None,
            bpmn_process_id: None,
            version: -1,
            variables: None,
            start_instructions: Vec::new(),
            tenant_id: None,
            operation_reference: None,
        }
    }

    /// Set the process key for this process instance.
    pub fn with_process_definition_key(self, key: i64) -> Self {
        CreateProcessInstanceBuilder {
            process_definition_key: Some(key),
            ..self
        }
    }

    /// Set the BPMN process id for this process instance.
    pub fn with_bpmn_process_id<T: Into<String>>(self, bpmn_process_id: T) -> Self {
        CreateProcessInstanceBuilder {
            bpmn_process_id: Some(bpmn_process_id.into()),
            ..self
        }
    }

    /// Set the version for this process instance.
    pub fn with_version(self, version: i32) -> Self {
        CreateProcessInstanceBuilder { version, ..self }
    }

    /// Use the latest process version for this process instance.
    pub fn with_latest_version(self) -> Self {
        CreateProcessInstanceBuilder {
            version: -1,
            ..self
        }
    }

    /// Set variables for this process instance.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        CreateProcessInstanceBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Set start instructions for this process instance.
    pub fn with_start_instructions(self, start_instructions: Vec<String>) -> Self {
        CreateProcessInstanceBuilder {
            start_instructions,
            ..self
        }
    }

    /// Set tenant ID for this process instance.
    pub fn with_tenant_id<T: Into<String>>(self, tenant_id: T) -> Self {
        CreateProcessInstanceBuilder {
            tenant_id: Some(tenant_id.into()),
            ..self
        }
    }

    /// Set operation reference for this process instance.
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        CreateProcessInstanceBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit this process instance to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), name = "create_process_instance", err)]
    pub async fn send(mut self) -> Result<CreateProcessInstanceResponse> {
        if self.process_definition_key.is_none() && self.bpmn_process_id.is_none() {
            return Err(Error::InvalidParameters(
                "`process_definition_key` or `pbmn_process_id` must be set",
            ));
        }
        let req = proto::CreateProcessInstanceRequest {
            process_definition_key: self.process_definition_key.unwrap_or(0),
            bpmn_process_id: self.bpmn_process_id.unwrap_or_default(),
            version: self.version,
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
            start_instructions: self
                .start_instructions
                .into_iter()
                .map(|var| proto::ProcessInstanceCreationStartInstruction { element_id: var })
                .collect(),
            tenant_id: self.tenant_id.unwrap_or_default(),
            operation_reference: self.operation_reference,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .create_process_instance(tonic::Request::new(req))
            .await?;

        Ok(CreateProcessInstanceResponse(res.into_inner()))
    }
}

/// Created process instance data.
#[derive(Debug)]
pub struct CreateProcessInstanceResponse(proto::CreateProcessInstanceResponse);

impl CreateProcessInstanceResponse {
    /// the key of the process definition which was used to create the process
    /// instance
    pub fn process_definition_key(&self) -> i64 {
        self.0.process_definition_key
    }

    /// the BPMN process ID of the process definition which was used to create the
    /// process instance
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the version of the process definition which was used to create the process
    /// instance
    pub fn version(&self) -> i32 {
        self.0.version
    }

    /// the unique identifier of the created process instance; to be used wherever
    /// a request needs a process instance key (e.g. CancelProcessInstanceRequest)
    pub fn process_instance_key(&self) -> i64 {
        self.0.process_instance_key
    }
}

/// Creates and starts an instance of the specified process with result.
///
/// Similar to [`CreateProcessInstanceBuilder`], creates and starts an instance of
/// the specified process. Unlike [`CreateProcessInstanceBuilder`], the response is
/// returned when the process is completed.
///
/// Note that only processes with none start events can be started through this
/// command.
#[derive(Debug)]
pub struct CreateProcessInstanceWithResultBuilder {
    client: Client,
    /// the unique key identifying the process definition (e.g. returned from a
    /// process in the DeployProcessResponse message)
    process_definition_key: Option<i64>,
    /// the BPMN process ID of the process definition
    bpmn_process_id: Option<String>,
    /// the version of the process; set to -1 to use the latest version
    version: i32,
    /// JSON document that will instantiate the variables for the root variable
    /// scope of the process instance; it must be a JSON object, as variables will
    /// be mapped in a key-value fashion. e.g. { "a": 1, "b": 2 } will create two
    /// variables, named "a" and "b" respectively, with their associated values. [{
    /// "a": 1, "b": 2 }] would not be a valid argument, as the root of the JSON
    /// document is an array and not an object.
    variables: Option<serde_json::Value>,
    /// timeout (in ms). the request will be closed if the process is not completed before
    /// the requestTimeout.
    ///
    /// if request_timeout = 0, uses the generic requestTimeout configured in the gateway.
    request_timeout: u64,
    /// list of names of variables to be included in
    /// [`CreateProcessInstanceWithResultResponse`]'s variables if empty, all visible
    /// variables in the root scope will be returned.
    fetch_variables: Vec<String>,
    /// List of start instructions. If empty (default) the process instance
    /// will start at the start event. If non-empty the process instance will apply start
    /// instructions after it has been created
    start_instructions: Vec<String>,
    /// the tenant id of the process definition
    tenant_id: Option<String>,
    /// a reference key chosen by the user and will be part of all records resulted from this operation
    operation_reference: Option<u64>,
}

impl CreateProcessInstanceWithResultBuilder {
    /// Create a new process instance builder
    pub fn new(client: Client) -> Self {
        CreateProcessInstanceWithResultBuilder {
            client,
            process_definition_key: None,
            bpmn_process_id: None,
            version: -1,
            variables: None,
            request_timeout: 0,
            fetch_variables: Vec::new(),
            start_instructions: Vec::new(),
            tenant_id: None,
            operation_reference: None,
        }
    }

    /// Set the process key for this process instance.
    pub fn with_process_definition_key(self, key: i64) -> Self {
        CreateProcessInstanceWithResultBuilder {
            process_definition_key: Some(key),
            ..self
        }
    }

    /// Set the BPMN process id for this process instance.
    pub fn with_bpmn_process_id<T: Into<String>>(self, bpmn_process_id: T) -> Self {
        CreateProcessInstanceWithResultBuilder {
            bpmn_process_id: Some(bpmn_process_id.into()),
            ..self
        }
    }

    /// Set the version for this process instance.
    pub fn with_version(self, version: i32) -> Self {
        CreateProcessInstanceWithResultBuilder { version, ..self }
    }

    /// Use the latest process version for this process instance.
    pub fn with_latest_version(self) -> Self {
        CreateProcessInstanceWithResultBuilder {
            version: -1,
            ..self
        }
    }

    /// Set variables for this process instance.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        CreateProcessInstanceWithResultBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Set variables for this process instance.
    pub fn with_fetch_variables(self, fetch_variables: Vec<String>) -> Self {
        CreateProcessInstanceWithResultBuilder {
            fetch_variables,
            ..self
        }
    }

    /// Set the result timeout for this process instance request.
    pub fn with_request_timeout(self, request_timeout: u64) -> Self {
        CreateProcessInstanceWithResultBuilder {
            request_timeout,
            ..self
        }
    }

    /// Set start instructions for this process instance.
    pub fn with_start_instructions(self, start_instructions: Vec<String>) -> Self {
        CreateProcessInstanceWithResultBuilder {
            start_instructions,
            ..self
        }
    }

    /// Set tenant ID for this process instance.
    pub fn with_tenant_id<T: Into<String>>(self, tenant_id: T) -> Self {
        CreateProcessInstanceWithResultBuilder {
            tenant_id: Some(tenant_id.into()),
            ..self
        }
    }

    /// Set operation reference for this process instance.
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        CreateProcessInstanceWithResultBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit this process instance to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), name = "create_process_instance_with_result", err)]
    pub async fn send(mut self) -> Result<CreateProcessInstanceWithResultResponse> {
        if self.process_definition_key.is_none() && self.bpmn_process_id.is_none() {
            return Err(Error::InvalidParameters(
                "`process_definition_key` or `pbmn_process_id` must be set",
            ));
        }
        let req = proto::CreateProcessInstanceWithResultRequest {
            request: Some(proto::CreateProcessInstanceRequest {
                process_definition_key: self.process_definition_key.unwrap_or(0),
                bpmn_process_id: self.bpmn_process_id.unwrap_or_default(),
                version: self.version,
                variables: self
                    .variables
                    .map_or(String::new(), |vars| vars.to_string()),
                start_instructions: self
                    .start_instructions
                    .into_iter()
                    .map(|var| proto::ProcessInstanceCreationStartInstruction { element_id: var })
                    .collect(),
                tenant_id: self.tenant_id.unwrap_or_default(),
                operation_reference: self.operation_reference,
            }),
            request_timeout: self.request_timeout as i64,
            fetch_variables: self.fetch_variables,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .create_process_instance_with_result(tonic::Request::new(req))
            .await?;

        Ok(CreateProcessInstanceWithResultResponse(res.into_inner()))
    }
}

/// Created process instance with result data.
#[derive(Debug)]
pub struct CreateProcessInstanceWithResultResponse(proto::CreateProcessInstanceWithResultResponse);

impl CreateProcessInstanceWithResultResponse {
    /// the key of the process definition which was used to create the process
    /// instance
    pub fn process_definition_key(&self) -> i64 {
        self.0.process_definition_key
    }

    /// the BPMN process ID of the process definition which was used to create the
    /// process instance
    pub fn bpmn_process_id(&self) -> &str {
        &self.0.bpmn_process_id
    }

    /// the version of the process definition which was used to create the process
    /// instance
    pub fn version(&self) -> i32 {
        self.0.version
    }

    /// the unique identifier of the created process instance; to be used wherever
    /// a request needs a process instance key (e.g. CancelProcessInstanceRequest)
    pub fn process_instance_key(&self) -> i64 {
        self.0.process_instance_key
    }

    /// Serialized JSON document that consists of visible variables in the root scope
    pub fn variables_str(&self) -> &str {
        &self.0.variables
    }

    /// JSON document consists of visible variables in the root scope
    pub fn variables(&self) -> serde_json::Value {
        serde_json::from_str(&self.0.variables).unwrap_or_else(|_| serde_json::json!({}))
    }

    /// Deserialize encoded json variables as a given type
    pub fn variables_as<'a, T: serde::de::Deserialize<'a>>(&'a self) -> Option<T> {
        serde_json::from_str::<'a, T>(&self.0.variables).ok()
    }
}

/// Cancels a running process instance.
#[derive(Debug)]
pub struct CancelProcessInstanceBuilder {
    client: Client,
    /// The unique key identifying the process instance (e.g. returned from a
    /// process in the [`CreateProcessInstanceResponse`] struct).
    process_instance_key: Option<i64>,
    operation_reference: Option<u64>,
}

impl CancelProcessInstanceBuilder {
    /// Create a new cancel process instance builder
    pub fn new(client: Client) -> Self {
        CancelProcessInstanceBuilder {
            client,
            process_instance_key: None,
            operation_reference: None,
        }
    }

    /// Set the process instance key.
    pub fn with_process_instance_key(self, key: i64) -> Self {
        CancelProcessInstanceBuilder {
            process_instance_key: Some(key),
            ..self
        }
    }

    /// Set operation reference
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        CancelProcessInstanceBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit this cancel process instance request to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), name = "cancel_process_instance", err)]
    pub async fn send(mut self) -> Result<CancelProcessInstanceResponse> {
        if self.process_instance_key.is_none() {
            return Err(Error::InvalidParameters(
                "`process_instance_key` must be set",
            ));
        }
        let req = proto::CancelProcessInstanceRequest {
            process_instance_key: self.process_instance_key.unwrap(),
            operation_reference: self.operation_reference,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .cancel_process_instance(tonic::Request::new(req))
            .await?;

        Ok(CancelProcessInstanceResponse(res.into_inner()))
    }
}

/// Canceled process instance data.
#[derive(Debug)]
pub struct CancelProcessInstanceResponse(proto::CancelProcessInstanceResponse);

/// Updates all the variables of a particular scope (e.g. process instance, flow
/// element instance) from the given JSON document.
#[derive(Debug)]
pub struct SetVariablesBuilder {
    client: Client,
    element_instance_key: Option<i64>,
    variables: Option<serde_json::Value>,
    local: bool,
    operation_reference: Option<u64>,
}

impl SetVariablesBuilder {
    /// Create a new set variables builder
    pub fn new(client: Client) -> Self {
        SetVariablesBuilder {
            client,
            element_instance_key: None,
            variables: None,
            local: false,
            operation_reference: None,
        }
    }

    /// Set the unique identifier of this element.
    ///
    /// can be the process instance key (as obtained during instance creation), or
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

    /// Set a reference key chosen by the user and will be part of all records resulted from this operation
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        SetVariablesBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit this set variables request to the configured Zeebe brokers.
    #[tracing::instrument(skip(self), name = "set_variables", err)]
    pub async fn send(mut self) -> Result<SetVariablesResponse> {
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
            operation_reference: self.operation_reference,
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
