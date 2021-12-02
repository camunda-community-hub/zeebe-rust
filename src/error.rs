use std::io;
use tonic::transport;

/// The result type returned by zeebe methods.
pub type Result<T> = std::result::Result<T, Error>;

/// The error type which is returned from zeebe processing failures.
///
/// The gRPC API for Zeebe is exposed through the gateway, which acts as a proxy
/// for the broker. Generally, this means that the client executes an remote
/// call on the gateway, which is then translated to special binary protocol
/// that the gateway uses to communicate with the broker.
///
/// As a result of this proxying, any errors which occur between the gateway and
/// the broker for which the client is not at fault (e.g. the gateway cannot
/// deserialize the broker response, the broker is unavailable, etc.) are
/// reported to the client using the following error codes.
///
/// * `GRPC_STATUS_RESOURCE_EXHAUSTED`: if the broker is receiving too many
///   requests more than what it can handle, it kicks off back-pressure and
///   rejects requests with this error code. In this case, it is possible to retry
///   the requests with an appropriate retry strategy. If you receive many such
///   errors with in a small time period, it indicates that the broker is
///   constantly under high load. It is recommended to reduce the rate of
///   requests. When the back-pressure kicks off, the broker may reject any
///   request except CompleteJob RPC and FailJob RPC. These requests are
///   white-listed for back-pressure and are always accepted by the broker even if
///   it is receiving requests above its limits.
/// * `GRPC_STATUS_UNAVAILABLE`: if the gateway itself is in an invalid state
///   (e.g. out of memory)
/// * `GRPC_STATUS_INTERNAL`: for any other internal errors that occurred between
///   the gateway and the broker.
///
/// This behavior applies to every single possible RPC; in these cases, it is possible that
/// retrying would succeed, but it is recommended to do so with an appropriate retry policy (e.g. a
/// combination of exponential backoff or jitter wrapped in a circuit breaker).
///
/// In the documentation below, the documented errors are business logic errors, meaning errors
/// which are a result of request processing logic, and not serialization, network, or other more
/// general errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error returned from invalid client gateway configuration.
    #[error("invalid gateway uri '{uri:?}': {message:?}")]
    InvalidGatewayUri {
        /// The error message.
        message: String,
        /// The invalid uri argument.
        uri: String,
    },
    /// GRPC transport errors.
    #[error(transparent)]
    Transport(#[from] transport::Error),
    /// GRPC result errors
    #[error(transparent)]
    GRPC(#[from] tonic::Status),
    /// File I/O errors when reading processes from disk.
    #[error("Invalid resource file {resource_file:?}: {source:?}")]
    FileIo {
        /// The specified resource file
        resource_file: String,
        /// The underlying `io::Error`.
        source: io::Error,
    },
    /// Invalid method parameters
    #[error("Invalid parameters: {0}")]
    InvalidParameters(&'static str),
    /// Job payload deserialization error
    #[error("Cannot deserialize variables to expected type: {0}")]
    DeserializationError(#[from] serde_json::Error),
    /// Missing worker state configuration
    #[error("Worker state is not configured, use `JobWorkerBuilder::with_state` while building worker for this job")]
    MissingWorkerStateConfig,
    #[error("Could not authenticate: {0}")]
    Auth(String),
}
