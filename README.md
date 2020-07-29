# Zeebe Rust Client

A rust client for defining, orchestrating, and monitoring business processes
across microservices using [Zeebe].

## What is Zeebe?

[Zeebe] is a workflow engine for microservices orchestration. Zeebe ensures
that, once started, flows are always carried out fully, retrying steps in case
of failures. Along the way, Zeebe maintains a complete audit log so that the
progress of flows can be monitored. Zeebe is fault tolerant and scales
seamlessly to handle growing transaction volumes.

[Zeebe]: https://zeebe.io

## Example

```rust
use std::collections::HashMap;
use zeebe::{Client, Job};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a zeebe client
    let mut client = Client::default();

    // Deploy a workflow
    client
        .deploy_workflow("order-process.bpmn")
        .send()
        .await?;

    // Create a new workflow instance
    let mut variables = HashMap::new();
    variables.insert("orderId", "31243");
    client
        .create_workflow_instance()
        .with_bpmn_process_id("order-process")
        .with_latest_version()
        .with_variables(serde_json::to_value(variables)?)
        .send()
        .await?;

    // Process a job type within the workflow
    client
        .job_worker()
        .with_job_type("payment-service")
        .with_handler(handle_job)
        .spawn()
        .await?;

    Ok(())
}

async fn handle_job(client: Client, job: Job) -> zeebe::Result<()> {
    /// your job processing logic...
    Ok(())
}
```
