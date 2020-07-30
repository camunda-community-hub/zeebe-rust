//! A rust client for defining, orchestrating, and monitoring business processes
//! across microservices using [Zeebe].
//!
//! ## What is Zeebe?
//!
//! [Zeebe] is a workflow engine for microservices orchestration. Zeebe ensures
//! that, once started, flows are always carried out fully, retrying steps in
//! case of failures. Along the way, Zeebe maintains a complete audit log so
//! that the progress of flows can be monitored. Zeebe is fault tolerant and
//! scales seamlessly to handle growing transaction volumes.
//!
//! [Zeebe]: https://zeebe.io
//!
//! ## Example
//!
//! ```no_run
//! use serde_json::json;
//! use zeebe::{Client, Job};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a zeebe client
//!     let mut client = Client::default();
//!
//!     // Deploy a workflow
//!     client
//!         .deploy_workflow("order-process.bpmn")
//!         .send()
//!         .await?;
//!
//!     // Create a new workflow instance
//!     client
//!         .create_workflow_instance()
//!         .with_bpmn_process_id("order-process")
//!         .with_latest_version()
//!         .with_variables(json!({"orderId": 31243}))
//!         .send()
//!         .await?;
//!
//!     // Process a job type within the workflow
//!     client
//!         .job_worker()
//!         .with_job_type("payment-service")
//!         .with_handler(handle_job)
//!         .spawn()
//!         .await?;
//!
//!     Ok(())
//! }
//!
//! async fn handle_job(mut client: Client, job: Job) {
//!     /// your job processing logic...
//!
//!     let _ = client.complete_job().with_job_key(job.key()).send().await;
//! }
//! ```
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]

pub(crate) mod client;
pub(crate) mod error;
pub(crate) mod job;
pub(crate) mod topology;
pub(crate) mod util;
pub(crate) mod worker;
pub(crate) mod workflow;

#[allow(clippy::all)]
pub(crate) mod proto {
    tonic::include_proto!("gateway_protocol");
}

pub use client::{Client, ClientConfig};
pub use error::{Error, Result};
pub use job::{
    CompleteJobBuilder, CompleteJobResponse, FailJobBuilder, FailJobResponse, ThrowErrorBuilder,
    ThrowErrorResponse, UpdateJobRetriesBuilder, UpdateJobRetriesResponse,
};
pub use topology::{BrokerInfo, Partition, TopologyBuilder, TopologyResponse};
pub use util::{
    PublishMessageBuilder, PublishMessageResponse, ResolveIncidentBuilder, ResolveIncidentResponse,
};
pub use worker::{Job, JobWorkerBuilder};
pub use workflow::{
    CancelWorkflowInstanceBuilder, CancelWorkflowInstanceResponse, CreateWorkflowInstanceBuilder,
    CreateWorkflowInstanceResponse, CreateWorkflowInstanceWithResultBuilder,
    CreateWorkflowInstanceWithResultResponse, DeployWorkflowBuilder, DeployWorkflowResponse,
    SetVariablesBuilder, SetVariablesResponse, WorkflowMetadata,
};
