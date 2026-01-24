use snafu::prelude::*;

/// Chalkydri's error type
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Invalid config"))]
    InvalidConfig,
    #[snafu(display("Failed to read config"))]
    FailedToReadConfig,
    #[snafu(display("Failed to map buffer"))]
    FailedToMapBuffer,
    #[snafu(display("Failed to pull sample"))]
    FailedToPullSample,

    #[snafu(display("No field layouts"))]
    NoFieldLayouts,
    #[snafu(display("No field layout selected"))]
    FieldLayoutNotSelected,
    #[snafu(display("Field layout does not exist: {id}"))]
    FieldLayoutDoesNotExist { id: String },

    #[snafu(display("Invalid AprilTag: {id}"))]
    InvalidTag { id: String },

    //#[snafu(display(""))]
}
