/// Chalkydri's error type
#[derive(Debug)]
pub enum Error {
    InvalidConfig,
    FailedToReadConfig,
    FailedToMapBuffer,
    FailedToPullSample,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(&self, f)
    }
}

impl std::error::Error for Error {
}
