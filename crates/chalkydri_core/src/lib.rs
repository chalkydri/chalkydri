//!
//! # Chalkydri Core
//!
//! This crate contains common data structures, traits, and utilities that can be reused.
//!

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

#[cfg(feature = "__toml")]
pub extern crate toml;
#[cfg(feature = "__json")]
pub extern crate serde_json;

#[cfg(feature = "preprocs")]
pub extern crate gstreamer;
#[cfg(feature = "preprocs")]
pub extern crate gstreamer_app;

mod error;
#[cfg(feature = "config")]
pub mod config;
#[cfg(feature = "ntables")]
mod ntables;
#[cfg(feature = "preprocs")]
pub mod preprocs;
#[cfg(feature = "subsystems")]
pub mod subsystems;

pub use error::Error;

pub mod prelude {
    pub use super::tokio::runtime::LocalRuntime;
    pub use super::parking_lot::{self, RwLock, Mutex, FairMutex};
    pub use super::tracing::{self, instrument, trace, debug, info, warn, error, Instrument};
    #[cfg(feature = "ntables")]
    pub use super::nt_client;
    #[cfg(feature = "preprocs")]
    pub use super::gstreamer;
    #[cfg(feature = "preprocs")]
    pub use super::gstreamer_app;

    pub use super::error::Error;
    pub use super::config::{self, Cfg, Config};
    #[cfg(feature = "ntables")]
    pub use super::ntables::Nt;
    #[cfg(feature = "preprocs")]
    pub use super::preprocs::{self, Preprocessor};
    #[cfg(feature = "subsystems")]
    pub use super::subsystems::{self, Subsystem};
}
