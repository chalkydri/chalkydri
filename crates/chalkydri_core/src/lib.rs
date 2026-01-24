//!
//! # Chalkydri Core
//!
//! This crate contains common data structures, traits, and utilities that can be reused.
//!

#![feature(coroutines, coroutine_trait)]

#![allow(
    // This is only used in Chalkydri code
    async_fn_in_trait,
)]
#![forbid(unsafe_code)]

pub extern crate tokio;
#[macro_use]
pub extern crate tracing;
pub extern crate parking_lot;

#[cfg(feature = "ntables")]
pub extern crate nt_client;

#[cfg(feature = "__json")]
pub extern crate serde_json;
#[cfg(feature = "__toml")]
pub extern crate toml;

#[cfg(feature = "preprocs")]
pub extern crate gstreamer;
#[cfg(feature = "preprocs")]
pub extern crate gstreamer_app;

#[cfg(feature = "config")]
pub mod config;
mod error;
#[cfg(feature = "ntables")]
mod ntables;
#[cfg(feature = "preprocs")]
pub mod preprocs;
#[cfg(feature = "subsystems")]
pub mod subsystems;

pub use error::Error;

pub mod prelude {
    #[cfg(feature = "preprocs")]
    pub use super::gstreamer;
    #[cfg(feature = "preprocs")]
    pub use super::gstreamer_app;
    #[cfg(feature = "ntables")]
    pub use super::nt_client;
    pub use super::parking_lot::{self, FairMutex, Mutex, RwLock};
    pub use super::tokio::runtime::LocalRuntime;
    pub use super::tracing::{self, Instrument, debug, error, info, instrument, trace, warn};

    pub use super::config::{self, Cfg, Config};
    pub use super::error::Error;
    #[cfg(feature = "ntables")]
    pub use super::ntables::Nt;
    #[cfg(feature = "preprocs")]
    pub use super::preprocs::{self, SubsysPreprocessor};
    #[cfg(feature = "subsystems")]
    pub use super::subsystems::{self, Subsystem};
}
