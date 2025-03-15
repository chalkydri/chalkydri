use rmp::decode::{bytes::BytesReadError, ValueReadError};
use std::error::Error;
use std::fmt::{self, Display};
use tokio_tungstenite::tungstenite::Error as TungsteniteError;

/// Custom error types for NetworkTables operations
#[derive(Debug)]
pub enum NtError {
    /// Error during connection establishment
    ConnectionError(String),
    /// Error with websocket communication
    WebsocketError(String),
    /// Error with MessagePack encoding/decoding
    MessagePackError(String),
    /// Error with JSON serialization/deserialization
    JsonError(String),
    /// Error with message sending
    SendError(String),
    /// Generic IO error
    IoError(String),
    /// Topic not found
    TopicNotFound(String),
    /// Binary frame parsing error
    BinaryFrameError,
    /// Lock acquisition error
    LockError(String),
    /// Other errors
    Other(String),
    NeedReconnect,
}

impl Display for NtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NtError::ConnectionError(msg) => write!(f, "Connection error: {msg}"),
            NtError::WebsocketError(msg) => write!(f, "Websocket error: {msg}"),
            NtError::MessagePackError(msg) => write!(f, "MessagePack error: {msg}"),
            NtError::JsonError(msg) => write!(f, "JSON error: {msg}"),
            NtError::SendError(msg) => write!(f, "Send error: {msg}"),
            NtError::IoError(msg) => write!(f, "IO error: {msg}"),
            NtError::TopicNotFound(msg) => write!(f, "Topic not found: {msg}"),
            NtError::BinaryFrameError => write!(f, "Failed to parse binary frame"),
            NtError::LockError(msg) => write!(f, "Lock error: {msg}"),
            NtError::Other(msg) => write!(f, "Error: {msg}"),
            NtError::NeedReconnect => write!(f, "Disconnected from NT server! Must reconnect"),
        }
    }
}

impl Error for NtError {}

// Implement From traits for various error types
impl From<std::io::Error> for NtError {
    fn from(err: std::io::Error) -> Self {
        NtError::IoError(err.to_string())
    }
}

impl From<TungsteniteError> for NtError {
    fn from(err: TungsteniteError) -> Self {
        NtError::WebsocketError(err.to_string())
    }
}

impl From<serde_json::Error> for NtError {
    fn from(err: serde_json::Error) -> Self {
        NtError::JsonError(err.to_string())
    }
}

impl From<rmp::encode::ValueWriteError> for NtError {
    fn from(err: rmp::encode::ValueWriteError) -> Self {
        NtError::MessagePackError(err.to_string())
    }
}

impl From<rmp::decode::ValueReadError> for NtError {
    fn from(err: rmp::decode::ValueReadError) -> Self {
        NtError::MessagePackError(format!("{:?}", err))
    }
}

impl From<ValueReadError<BytesReadError>> for NtError {
    fn from(err: ValueReadError<BytesReadError>) -> Self {
        NtError::MessagePackError(format!("{:?}", err))
    }
}

impl From<tokio_tungstenite::tungstenite::http::Error> for NtError {
    fn from(err: tokio_tungstenite::tungstenite::http::Error) -> Self {
        NtError::ConnectionError(err.to_string())
    }
}

// Type alias for Result with NtError
pub type Result<T> = std::result::Result<T, NtError>;
