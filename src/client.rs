use crate::oauth::AuthInterceptor;
use crate::resources::{DeleteResourceBuilder, DeployResourceBuilder};
use crate::{
    error::{Error, Result},
    job::{CompleteJobBuilder, FailJobBuilder, ThrowErrorBuilder, UpdateJobRetriesBuilder},
    oauth::OAuthConfig,
    process::{
        CancelProcessInstanceBuilder, CreateProcessInstanceBuilder,
        CreateProcessInstanceWithResultBuilder, DeployProcessBuilder, SetVariablesBuilder,
    },
    proto::gateway_client::GatewayClient,
    topology::TopologyBuilder,
    util::{PublishMessageBuilder, ResolveIncidentBuilder},
    worker::{auto_handler::Extensions, JobWorkerBuilder},
};
use std::env;
use std::fmt::Debug;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tonic::codegen::InterceptedService;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(45);
const CA_CERTIFICATE_PATH: &str = "ZEEBE_CA_CERTIFICATE_PATH";
const ADDRESS: &str = "ZEEBE_ADDRESS";
const HOST: &str = "ZEEBE_HOST";
const DEFAULT_ADDRESS_HOST: &str = "http://127.0.0.1";
const PORT: &str = "ZEEBE_PORT";
const DEFAULT_ADDRESS_PORT: &str = "26500";

/// Client used to communicate with Zeebe.
#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) gateway_client: GatewayClient<InterceptedService<Channel, AuthInterceptor>>,
    pub(crate) auth_interceptor: AuthInterceptor,
    pub(crate) current_job_key: Option<i64>,
    pub(crate) current_job_extensions: Option<Arc<Extensions>>,
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

    /// Create a new client from environment variables
    pub fn from_env() -> Result<Self> {
        Client::from_config(ClientConfig::from_env()?)
    }

    /// Build a new Zeebe client from a given configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use zeebe::{Client, ClientConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let endpoints = vec!["http://0.0.0.0:26500".to_string()];
    ///
    /// let client = Client::from_config(ClientConfig::with_endpoints(endpoints));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// with TLS (see [the ClientTlsConfig docs] for configuration):
    ///
    /// [the ClientTlsConfig docs]: tonic::transport::ClientTlsConfig
    ///
    /// ```
    /// use zeebe::{Client, ClientConfig};
    /// use tonic::transport::ClientTlsConfig;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let endpoints = vec!["http://0.0.0.0:26500".to_string()];
    /// let tls = ClientTlsConfig::new();
    ///
    /// let client = Client::from_config(ClientConfig {
    ///     endpoints,
    ///     tls: Some(tls),
    ///     auth: None,
    /// })?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(config: ClientConfig) -> Result<Self> {
        let channel = Self::build_channel(config.endpoints, config.tls)?;

        let auth_interceptor = if let Some(auth_config) = config.auth {
            AuthInterceptor::init(auth_config)?
        } else {
            AuthInterceptor::default()
        };

        let gateway_client = GatewayClient::with_interceptor(channel, auth_interceptor.clone());

        Ok(Client {
            gateway_client,
            auth_interceptor,
            current_job_key: None,
            current_job_extensions: None,
        })
    }

    /// Future that resolves when the auth interceptor is initialized.
    pub async fn auth_initialized(&self) -> Result<()> {
        self.auth_interceptor.auth_initialized().await
    }

    /// Obtains the current topology of the cluster the gateway is part of.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let topology = client.topology().send().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn topology(&self) -> TopologyBuilder {
        TopologyBuilder::new(self.clone())
    }

    /// Deploys one or more processes to Zeebe. Note that this is an atomic call,
    /// i.e. either all processes are deployed, or none of them are.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let process = client
    ///     .deploy_process()
    ///     .with_resource_file("path/to/process.bpmn")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn deploy_process(&self) -> DeployProcessBuilder {
        DeployProcessBuilder::new(self.clone())
    }

    /// Deploys a single resource to Zeebe
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let deployment = client
    ///     .deploy_resource()
    ///     .with_resource_name("my-name")
    ///     .with_content(vec![1, 2, 3])
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn deploy_resource(&self) -> DeployResourceBuilder {
        DeployResourceBuilder::new(self.clone())
    }

    /// Delete a single resource from Zeebe
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let removed = client
    ///    .delete_resource()
    ///    .with_resource_key(123)
    ///    .send()
    ///    .await?;
    /// # Ok(())
    /// # }
    pub fn delete_resource(&self) -> DeleteResourceBuilder {
        DeleteResourceBuilder::new(self.clone())
    }

    /// Creates and starts an instance of the specified process.
    ///
    /// The process definition to use to create the instance can be specified
    /// either using its unique key (as returned by [`deploy_process`]), or using the
    /// BPMN process ID and a version. Pass -1 as the version to use the latest
    /// deployed version.
    ///
    /// Note that only processes with none start events can be started through this
    /// command.
    ///
    /// [`deploy_process`]: Client::deploy_process
    ///
    ///  # Examples
    ///
    /// ```no_run
    /// use serde_json::json;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let process_instance = client
    ///     .create_process_instance()
    ///     .with_bpmn_process_id("example-process")
    ///     .with_latest_version()
    ///     .with_variables(json!({"myData": 31243}))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn create_process_instance(&self) -> CreateProcessInstanceBuilder {
        CreateProcessInstanceBuilder::new(self.clone())
    }

    /// Similar to [`create_process_instance`], creates and starts an instance of
    /// the specified process_
    ///
    /// Unlike [`create_process_instance`], the response is returned when the
    /// process_is completed.
    ///
    /// Note that only processes with none start events can be started through this
    /// command.
    ///
    /// [`create_process_instance`]: Client::create_process_instance
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use serde_json::json;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let process_instance_with_result = client
    ///     .create_process_instance_with_result()
    ///     .with_bpmn_process_id("example-process")
    ///     .with_latest_version()
    ///     .with_variables(json!({"myData": 31243}))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn create_process_instance_with_result(&self) -> CreateProcessInstanceWithResultBuilder {
        CreateProcessInstanceWithResultBuilder::new(self.clone())
    }

    /// Cancels a running process instance.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// // process instance key, e.g. from a `CreateProcessInstanceResponse`.
    /// let process_instance_key = 2251799813687287;
    ///
    /// let canceled = client
    ///     .cancel_process_instance()
    ///     .with_process_instance_key(process_instance_key)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn cancel_process_instance(&self) -> CancelProcessInstanceBuilder {
        CancelProcessInstanceBuilder::new(self.clone())
    }

    /// Updates all the variables of a particular scope (e.g. process instance,
    /// flow element instance) from the given JSON document.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use serde_json::json;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// // process instance key, e.g. from a `CreateProcessInstanceResponse`.
    /// let element_instance_key = 2251799813687287;
    ///
    /// let set_variables = client
    ///     .set_variables()
    ///     .with_element_instance_key(element_instance_key)
    ///     .with_variables(json!({"myNewKey": "myValue"}))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn set_variables(&self) -> SetVariablesBuilder {
        SetVariablesBuilder::new(self.clone())
    }

    /// Create a new job worker builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use zeebe::{Client, Job};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new();
    ///
    /// client
    ///     .job_worker()
    ///     .with_job_type("my-service")
    ///     .with_handler(handle_job)
    ///     .run()
    ///     .await?;
    ///
    /// // job handler function
    /// async fn handle_job(client: Client, job: Job) {
    ///     // processing work...
    ///
    ///     let _ = client.complete_job().with_job_key(job.key()).send().await;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn job_worker(&self) -> JobWorkerBuilder {
        JobWorkerBuilder::new(self.clone())
    }

    /// Completes a job with the given payload, which allows completing the
    /// associated service task.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// // typically obtained from `job.key()`;
    /// let job_key = 2251799813687287;
    ///
    /// let completed_job = client
    ///     .complete_job()
    ///     .with_job_key(job_key)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn complete_job(&self) -> CompleteJobBuilder {
        CompleteJobBuilder::new(self.clone())
    }

    /// Marks the job as failed.
    ///
    /// If the `retries` argument is positive, then the job will be immediately
    /// activatable again, and a worker could try again to process it. If it is zero
    /// or negative however, an incident will be raised, tagged with the given
    /// `error_message`, and the job will not be activatable until the incident is
    /// resolved.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// // typically obtained from `job.key()`;
    /// let job_key = 2251799813687287;
    ///
    /// let failed_job = client
    ///     .fail_job()
    ///     .with_job_key(job_key)
    ///     .with_error_message("something went wrong.")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn fail_job(&self) -> FailJobBuilder {
        FailJobBuilder::new(self.clone())
    }

    /// Updates the number of retries a job has left.
    ///
    /// This is mostly useful for jobs that have run out of retries, should the
    /// underlying problem be solved.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// // typically obtained from `job.key()`;
    /// let job_key = 2251799813687287;
    ///
    /// let updated = client
    ///     .update_job_retries()
    ///     .with_job_key(job_key)
    ///     .with_retries(2)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn update_job_retries(&self) -> UpdateJobRetriesBuilder {
        UpdateJobRetriesBuilder::new(self.clone())
    }

    /// Throw an error to indicate that a business error has occurred while
    /// processing the job.
    ///
    /// The error is identified by an error code and is handled by an error catch
    /// event in the process with the same error code.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// // typically obtained from `job.key()`;
    /// let job_key = 2251799813687287;
    ///
    /// let error = client
    ///     .throw_error()
    ///     .with_job_key(job_key)
    ///     .with_error_message("something went wrong")
    ///     .with_error_code("E2505")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn throw_error(&self) -> ThrowErrorBuilder {
        ThrowErrorBuilder::new(self.clone())
    }

    /// Publishes a single message. Messages are published to specific partitions
    /// computed from their correlation keys.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use serde_json::json;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let message = client
    ///     .publish_message()
    ///     .with_name("myEvent")
    ///     .with_variables(json!({"someKey": "someValue"}))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn publish_message(&self) -> PublishMessageBuilder {
        PublishMessageBuilder::new(self.clone())
    }

    /// Resolves a given incident.
    ///
    /// This simply marks the incident as resolved; most likely a call to
    /// [`update_job_retries`] will be necessary to actually resolve the problem,
    /// followed by this call.
    ///
    /// [`update_job_retries`]: Client::update_job_retries
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = zeebe::Client::new();
    ///
    /// let incident_key = 2251799813687287;
    ///
    /// let resolved = client
    ///     .resolve_incident()
    ///     .with_incident_key(incident_key)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    pub fn resolve_incident(&self) -> ResolveIncidentBuilder {
        ResolveIncidentBuilder::new(self.clone())
    }

    fn build_channel(endpoints: Vec<String>, tls: Option<ClientTlsConfig>) -> Result<Channel> {
        let endpoints = endpoints
            .into_iter()
            .map(|uri| {
                Channel::from_shared(uri.clone())
                    .map_err(|err| Error::InvalidGatewayUri {
                        uri,
                        message: err.to_string(),
                    })
                    .map(|channel| {
                        channel
                            .timeout(DEFAULT_REQUEST_TIMEOUT)
                            .keep_alive_timeout(DEFAULT_KEEP_ALIVE)
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
///
/// See [the ClientTlsConfig docs] for tls configuration.
///
/// [the ClientTlsConfig docs]: tonic::transport::ClientTlsConfig
///
/// # Examples
///
/// ```
/// let endpoints = vec!["http://127.0.0.1:26500".to_string()];
///
/// let config = zeebe::ClientConfig {
///     endpoints,
///     tls: None,
///     auth: None,
/// };
/// ```
#[derive(Debug)]
pub struct ClientConfig {
    /// The endpoints the client should connect to
    pub endpoints: Vec<String>,
    /// TLS configuration
    pub tls: Option<ClientTlsConfig>,
    /// OAuth config
    pub auth: Option<OAuthConfig>,
}

impl ClientConfig {
    /// Get client config from environment
    pub fn from_env() -> Result<Self> {
        let tls = if let Ok(ca_path) = env::var(CA_CERTIFICATE_PATH) {
            let pem = fs::read_to_string(ca_path).map_err(|err| Error::Auth(err.to_string()))?;
            let cert = Certificate::from_pem(pem);

            Some(ClientTlsConfig::new().ca_certificate(cert))
        } else {
            None
        };

        let address = if let Ok(gateway_host) = env::var(HOST) {
            if let Ok(gateway_port) = env::var(PORT) {
                format!("{}:{}", gateway_host, gateway_port)
            } else {
                format!("{}:{}", gateway_host, DEFAULT_ADDRESS_PORT)
            }
        } else if let Ok(gateway_port) = env::var(PORT) {
            format!("{}:{}", DEFAULT_ADDRESS_HOST, gateway_port)
        } else if let Ok(gateway_address) = env::var(ADDRESS) {
            gateway_address
        } else {
            format!("{}:{}", DEFAULT_ADDRESS_HOST, DEFAULT_ADDRESS_PORT)
        };

        let auth = if OAuthConfig::should_use_env_config() {
            Some(OAuthConfig::from_env()?)
        } else {
            None
        };

        Ok(ClientConfig {
            endpoints: vec![address],
            tls,
            auth,
        })
    }

    /// Set the grpc endpoints the client should connect to.
    pub fn with_endpoints(endpoints: Vec<String>) -> Self {
        ClientConfig {
            endpoints,
            tls: None,
            auth: None,
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        let default_address = format!("{}:{}", DEFAULT_ADDRESS_HOST, DEFAULT_ADDRESS_PORT);
        ClientConfig::with_endpoints(vec![default_address])
    }
}
