use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use thiserror::Error;
use zeebe::{Client, Data, State};

// Any error that implements `std::error::Error`
#[derive(Error, Debug)]
enum MyError {}

// A type that can be deserialized from job variables data
#[derive(Deserialize)]
struct MyJobData {
    increment: u32,
}

// A result type that can be serialized as job success variables
#[derive(Serialize)]
struct MyJobResult {
    result: u32,
}

// Optional worker state that persists across jobs
struct JobState {
    total: RwLock<u32>,
}

// Job handler with arbitrary number of parameters that can be extracted
async fn handle_job(
    job_data: Data<MyJobData>,
    job_state: State<JobState>,
) -> Result<MyJobResult, MyError> {
    let current_total = *job_state.total.read().unwrap();
    let new_total = current_total + job_data.increment;

    *job_state.total.write().unwrap() = new_total;

    Ok(MyJobResult { result: new_total })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // Initialize the worker state
    let job_state = JobState {
        total: RwLock::new(0),
    };

    // Run the job
    client
        .job_worker()
        .with_job_type("my-job-type")
        .with_auto_handler(handle_job)
        .with_state(job_state)
        .run()
        .await?;

    Ok(())
}
