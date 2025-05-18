//use transforms::errors::TransformError;

use minint::NtError;

/// Chalkydri's error type
#[derive(Debug)]
pub enum Error {
    InvalidConfig,
    FailedToReadConfig,
    FailedToMapBuffer,
    FailedToPullSample,
    //FailedToAddTransform(tokio::sync::mpsc::error::SendError<transforms::Transform>),
    //FailedToGetPose(TransformError),
    NoFieldLayouts,
    FieldLayoutNotSelected,
    FieldLayoutDoesNotExist,

    InvalidTag,
    NtError(NtError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(&self, f)
    }
}

impl std::error::Error for Error {}
