//!
//! Chalkydri core
//!

// Unsafe code is NOT allowed in Chalkydri core.
// If unsafe code is required, it should be part of a different crate.
#![forbid(unsafe_code)]

#[macro_use]
extern crate log;
extern crate tokio;
extern crate env_logger;
extern crate fast_image_resize;
#[cfg(feature = "libcamera")]
extern crate libcamera;
#[cfg(feature = "mjpeg")]
extern crate mozjpeg;
extern crate ril;
extern crate minint;
#[macro_use]
extern crate actix_web;
extern crate utoipa as utopia;
#[macro_use]
extern crate serde;
#[cfg(feature = "apriltags")]
extern crate chalkydri_apriltags;
#[cfg(feature = "ml")]
extern crate tfledge;
#[cfg(feature = "python")]
extern crate pyo3;

#[cfg(feature = "libcamera")]
mod cameras;
mod subsys;
mod config;
mod api;
mod utils;
mod logger;

use std::{error::Error, time::Duration};
use minint::NtConn;
use tokio::runtime::Runtime;

use crate::{api::run_api, utils::gen_team_ip};

/// A processing subsystem
///
/// Subsystems implement different computer vision tasks, such as AprilTags or object detection.
///
/// A subsystem should be generic, not something that is only used for some specific aspect of a
/// game.
/// For example, note detection for the 2024 game, Crescendo, would go under the object detection
/// subsystem, rather than a brand new subsystem.
///
/// Make sure to pay attention to and respect each subsystem's documentation and structure.
pub trait Subsystem: Sized {
    /// Initialize the subsystem
    async fn init() -> Result<Self, Box<dyn Error>>;
    /// Run the subsystem
    async fn run(&self);
    /// Shutdown the subsystem
    async fn shutdown(self);
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logger
    env_logger::init();

    info!("Chalkydri starting up...");

    let roborio_ip = gen_team_ip(4533).expect("failed to generate team ip");
    // Generate a random device id
    let dev_id = fastrand::u32(..);

    // Build a tokio runtime
    let rt = Runtime::new().unwrap();

    // Enter the tokio rt
    rt.block_on(async {
        // Attempt to connect to the NT server, retrying until successful

        let nt: NtConn;

        let mut retry = false;

        loop {
            match NtConn::new(roborio_ip, format!("chalkydri{dev_id}")).await {
                Ok(conn) => {
                    nt = conn;
                    break;
                },
                Err(err) => {
                    if !retry {
                        error!("Error connecting to NT server: {err:?}");
                        retry = true;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                },
            }
        }

        info!("Connected to NT server at {roborio_ip:?} successfully!");

        // Have to let NT topics get dropped before calling nt.stop()
        {
            run_api(nt.clone()).await;
        }

        // Shut down NT connection
        nt.stop();
    });

    Ok(())
}
