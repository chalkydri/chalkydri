//!
//! Chalkydri's custom logger
//!

/// Custom [log::Log] implementation
///
/// We need a custom logger to get log messages back to the driver station.
pub struct Logger {
}
impl log::Log for Logger {
    fn log(&self, record: &log::Record) {
        //
    }
}
