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

#[cfg(feature = "__json")]
pub extern crate serde_json;
#[cfg(feature = "__toml")]
pub extern crate toml;

#[cfg(feature = "config")]
pub mod config;
mod error;

pub use error::Error;

pub mod prelude {
    pub use super::parking_lot::{self, FairMutex, Mutex, RwLock};
    pub use super::tracing::{self, Instrument, debug, error, info, instrument, trace, warn};

    pub use super::config::{self, Cfg, Config};
    pub use super::error::Error;
}
