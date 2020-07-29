use std::io;
use tonic::transport;

/// The result type returned by zeebe methods.
pub type Result<T> = std::result::Result<T, Error>;

/// The error type which is returned from zeebe processing failures.
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
    /// GRPC transport errors
    #[error(transparent)]
    Transport(#[from] transport::Error),
    /// GRPC result errors
    #[error(transparent)]
    GRPC(#[from] tonic::Status),
    /// File I/O errors when reading workflows from disk.
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
}
