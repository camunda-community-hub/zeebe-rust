use std::fmt;
use std::ops;
use std::sync::Arc;

/// Worker state that persists across job [`auto handler`] invocations.
///
/// [`auto handler`]: crate::JobWorkerBuilder::with_auto_handler
///
/// # Examples
///
/// ```no_run
/// use futures::future;
/// use serde::Serialize;
/// use std::cell::Cell;
/// use thiserror::Error;
/// use zeebe::{Client, State};
///
/// #[derive(Error, Debug)]
/// enum MyError {}
///
/// #[derive(Serialize)]
/// struct MyJobResult {
///     result: u32,
/// }
///
/// struct MyJobState {
///     total: Cell<u32>,
/// }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = Client::default();
///
/// let job_state = MyJobState {
///     total: Cell::new(0),
/// };
///
/// let _job = client
///     .job_worker()
///     .with_job_type("my-job-type")
///     .with_auto_handler(|my_job_state: State<MyJobState>| {
///         future::ok::<_, MyError>(MyJobResult { result: 42 })
///     })
///     .with_state(job_state)
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct State<T>(Arc<T>);

impl<T> State<T>
where
    T: Send + 'static,
{
    /// Create new `State` instance.
    pub fn new(state: T) -> State<T> {
        State(Arc::new(state))
    }

    /// Get reference to inner app data.
    pub fn get_ref(&self) -> &T {
        self.0.as_ref()
    }

    /// Convert to the internal Arc<T>
    pub fn into_inner(self) -> Arc<T> {
        self.0
    }
}

impl<T> ops::Deref for State<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Arc<T> {
        &self.0
    }
}

impl<T> Clone for State<T> {
    fn clone(&self) -> State<T> {
        State(self.0.clone())
    }
}

impl<T> From<Arc<T>> for State<T> {
    fn from(arc: Arc<T>) -> Self {
        State(arc)
    }
}

impl<T> fmt::Debug for State<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "State: {:?}", self.0)
    }
}

impl<T> fmt::Display for State<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
