//!
//! Chalkydri's custom logger
//!

#[cfg(feature = "rerun")]
use crate::Rerun;

use env_logger::Builder;
use log::Log;
#[cfg(feature = "rerun")]
use re_sdk::{external::re_log, RecordingStream};
#[cfg(feature = "rerun")]
use re_types::{archetypes::TextLog, components::TextLogLevel};

/// Custom [log::Log] implementation based on `rerun`'s
///
/// Implements a [`log::Log`] that forwards all events to the Rerun SDK.
#[derive(Debug)]
pub struct Logger {
    logger: Option<env_logger::Logger>,
    path_prefix: Option<String>,
}
impl Logger {
    /// Returns a new [`Logger`] that forwards all events to the specified [`RecordingStream`].
    pub fn new() -> Self {
        Self {
            logger: None,
            path_prefix: None,
        }
    }

    /// Configures the [`Logger`] to prefix the specified `path_prefix` to all events.
    #[inline]
    pub fn with_path_prefix(mut self, path_prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(path_prefix.into());
        self
    }

    /// Configures the [`Logger`] to filter events.
    ///
    /// This uses the familiar [env_logger syntax].
    ///
    /// If you don't call this, the [`Logger`] will parse the `RUST_LOG` environment variable
    /// instead when you [`Logger::init`] it.
    ///
    /// [env_logger syntax]: https://docs.rs/env_logger/latest/env_logger/index.html#enabling-logging
    #[inline]
    pub fn with_filter(mut self, filter: impl AsRef<str>) -> Self {
        self.logger = Some(Builder::new().parse_filters(filter.as_ref()).build());
        self
    }

    /// Sets the [`Logger`] as global logger.
    ///
    /// All calls to [`log`] macros will go through this [`Logger`] from this point on.
    pub fn init(mut self) -> Result<(), log::SetLoggerError> {
        if self.logger.is_none() {
            #[cfg(feature = "rerun")]
            {
                self.logger = Some(
                    Builder::new()
                        .parse_filters(&re_log::default_log_filter())
                        .build(),
                );
            }
            #[cfg(not(feature = "rerun"))]
            {
                self.logger = Some(Builder::new().parse_default_env().build());
            }
        }

        // NOTE: We will have to make filtering decisions on a per-crate/module basis, therefore
        // there is no global filtering ceiling.
        log::set_max_level(log::LevelFilter::max());
        log::set_boxed_logger(Box::new(self))
    }
}
impl log::Log for Logger {
    #[inline]
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.logger
            .as_ref()
            .map_or(true, |filter| filter.enabled(metadata))
    }

    #[inline]
    fn log(&self, record: &log::Record<'_>) {
        if !self
            .logger
            .as_ref()
            .map_or(true, |filter| filter.matches(record))
        {
            return;
        }

        // Do normal logging to console
        self.logger.as_ref().map(|logger| logger.log(record));

        // Do logging to Rerun
        #[cfg(feature = "rerun")]
        {
            let target = record.metadata().target().replace("::", "/");
            let ent_path = if let Some(path_prefix) = self.path_prefix.as_ref() {
                format!("{path_prefix}/{target}")
            } else {
                target
            };

            let level = log_level_to_rerun_level(record.metadata().level());

            let body = format!("{}", record.args());

            Rerun
                .log(ent_path, &TextLog::new(body).with_level(level))
                .ok(); // ignore error
        }
    }

    #[inline]
    fn flush(&self) {
        #[cfg(feature = "rerun")]
        Rerun.flush_blocking();
    }
}
impl Drop for Logger {
    fn drop(&mut self) {
        self.flush();
    }
}

// ---

#[cfg(feature = "rerun")]
fn log_level_to_rerun_level(lvl: log::Level) -> TextLogLevel {
    match lvl {
        log::Level::Error => TextLogLevel::ERROR,
        log::Level::Warn => TextLogLevel::WARN,
        log::Level::Info => TextLogLevel::INFO,
        log::Level::Debug => TextLogLevel::DEBUG,
        log::Level::Trace => TextLogLevel::TRACE,
    }
    .into()
}
