use std::fmt;
use std::ops;

/// Data that can be extracted from JSON encoded job variables in [`auto handlers`].
///
/// [`auto handlers`]: struct.JobWorkerBuilder.html#method.with_auto_handler
///
/// # Examples
///
/// ```no_run
/// use futures::future;
/// use serde::{Deserialize, Serialize};
/// use std::cell::Cell;
/// use thiserror::Error;
/// use zeebe::{Client, Data};
///
/// #[derive(Error, Debug)]
/// enum MyError {}
///
/// #[derive(Serialize)]
/// struct MyJobResult {
///     result: u32,
/// }
///
/// #[derive(Deserialize)]
/// struct MyJobData {
///     some_key: String,
/// }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = Client::default();
///
/// let _job = client
///     .job_worker()
///     .with_job_type("my-job-type")
///     .with_auto_handler(|my_job_data: Data<MyJobData>| {
///         future::ok::<_, MyError>(MyJobResult { result: 42 })
///     })
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct Data<T>(pub T);

impl<T> Data<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> ops::Deref for Data<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> ops::DerefMut for Data<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> fmt::Debug for Data<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Json: {:?}", self.0)
    }
}

impl<T> fmt::Display for Data<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
