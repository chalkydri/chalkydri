//!
//! # Chalkydri
//!
//! This crate contains Chalkydri itself.
//!
//! This code runs on the vision coprocessor(s) and does all the heavy lifting.
//!

#![feature(coroutines, coroutine_trait)]
#![allow(unreachable_code)]
#![deny(
    unused_must_use,
    clippy::infinite_iter,
    clippy::infinite_loop,
    clippy::unconditional_recursion,
    clippy::while_immutable_condition
)]

// These deps are needed no matter what
extern crate tracing;
#[macro_use]
extern crate serde;

use chalkydri::cameras;
use chalkydri::subsystems;
use chalkydri::utils;

pub use subsystems::apriltags::AprilAdapter;

pub use cameras::pipeline::CamPipeline;
pub use subsystems::calibration::Calibrator;
use chalkydri_core::{
    config::{Cfg, Config},
    prelude::config,
};
use cu29::prelude::*;
use mimalloc::MiMalloc;

use std::{
    error::Error,
    path::{Path, PathBuf},
    str::FromStr,
};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

use cu29_helpers::basic_copper_setup;

#[copper_runtime(config = "copperconfig.ron")]
struct App {}

#[tokio::main(worker_threads = 16)]
async fn main() -> Result<(), Box<dyn Error>> {
    println!(
        r#"
    ))       ((       ___       __               _    __  ___
   / /       \ \     |    |  | |  | |   | / \ / | \  |  |  |
  / \\   _   / /\    |    |__| |__| |   |/   V  |  | |__|  |
 / / \__/6\>_/ \ \   |    |  | |  | |   |\   |  |  | | \   |
(  __          __ )  |___ |  | |  | |__ | \  |  |_/  |  \ _|_
 \_____     _____/        
      //////\             High-performance vision system
      UUUUUUU                FRC Team 4533 - Phoenix
"#
    );

    // Set up logging
    #[cfg(not(feature = "tokio-console"))]
    {
        let filter = EnvFilter::from_default_env();
        let layer = tracing_subscriber::fmt::layer().with_filter(filter);
        tracing_subscriber::registry().with(layer).init();
    }

    #[cfg(feature = "tokio-console")]
    console_subscriber::init();

    tracing::info!("starting up...");

    // Try a few different paths for the config file, exiting early if we find one that exists
    let mut path = Path::new("/boot/chalkydri.toml");
    if !path.exists() {
        path = Path::new("/etc/chalkydri.toml");
        if !path.exists() {
            path = Path::new("./chalkydri.toml");
        }
    }
    tracing::trace!("loading config from '{path:?}'");

    // If all else fails, we'll just use a default configuration
    (*Cfg.write()) = Config::load(path).unwrap_or_default();

    // Disable BS kernel modules
    let _ = rustix::system::delete_module(c"rpivid_hevc", 0);
    let _ = rustix::system::delete_module(c"pisp_be", 0);

    // Initialize GStreamer
    match gstreamer::init() {
        Ok(()) => {
            tracing::debug!("initialized gstreamer");
        }
        Err(e) => {
            panic!("gstreamer failed to initialize: {e:?}");
        }
    }

    let pathbuf = PathBuf::from_str("chalkydri.copper".into()).unwrap();
    let copper_ctx = basic_copper_setup(pathbuf.as_path(), None, true, None).unwrap();

    let clock = copper_ctx.clock;

    let mut app = App::new(clock.clone(), copper_ctx.unified_logger.clone(), None)
        .expect("failed to create runtime");

    app.run().unwrap();

    Ok(())
}
