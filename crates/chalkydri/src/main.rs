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
#[macro_use]
extern crate tracing;
#[macro_use]
extern crate serde;
extern crate tokio;

#[cfg(feature = "tokio-console")]
extern crate console_subscriber;

// Web server and OpenAPI documentation generator
#[cfg(feature = "web")]
extern crate actix_web;
#[cfg(feature = "web")]
extern crate utoipa as utopia;

// Apriltag stuff
#[cfg(feature = "capriltags")]
extern crate apriltag;
#[cfg(feature = "apriltags")]
extern crate chalkydri_apriltags;

#[cfg(feature = "ml")]
extern crate tfledge;

//extern crate sophus_lie;
//extern crate sophus_autodiff;

#[cfg(feature = "web")]
mod api;
mod cameras;
//mod pose;
mod subsystems;
mod utils;

#[cfg(feature = "web")]
use api::run_api;
use cameras::CamManager;
use chalkydri_core::prelude::*;
use mimalloc::MiMalloc;
#[cfg(feature = "rerun")]
use re_sdk::{MemoryLimit, RecordingStream};
#[cfg(feature = "rerun_web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "rerun")]
use re_ws_comms::RerunServerPort;
use std::{error::Error, path::Path};
use tokio::sync::mpsc;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

// mimalloc is an excellent general purpose allocator
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(feature = "rerun")]
#[allow(non_upper_case_globals)]
static Rerun: Lazy<RecordingStream> = Lazy::new(|| {
    #[cfg(feature = "rerun_web_viewer")]
    re_sdk::RecordingStreamBuilder::new("chalkydri")
        .serve_web(
            "0.0.0.0",
            WebViewerServerPort(8080),
            RerunServerPort(6969),
            MemoryLimit::from_bytes(10_000_000),
            true,
        )
        .unwrap()
        .into()
});

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

    info!("starting up...");

    // Try a few different paths for the config file, exiting early if we find one that exists
    let mut path = Path::new("/boot/chalkydri.toml");
    if !path.exists() {
        path = Path::new("/etc/chalkydri.toml");
        if !path.exists() {
            path = Path::new("./chalkydri.toml");
        }
    }
    trace!("loading config from '{path:?}'");

    // If all else fails, we'll just use a default configuration
    (*Cfg.write()) = Config::load(path).unwrap_or_default();

    // Disable BS kernel modules
    let _ = rustix::system::delete_module(c"rpivid_hevc", 0);
    let _ = rustix::system::delete_module(c"pisp_be", 0);

    // Initialize GStreamer
    match gstreamer::init() {
        Ok(()) => {
            debug!("initialized gstreamer");
        }
        Err(e) => {
            panic!("gstreamer failed to initialize: {e:?}");
        }
    }

    // Create the shutdown channel
    let (tx, mut rx) = mpsc::channel::<()>(1);
    // Spawn the camera manager
    let (cam_man, runner) = CamManager::new(Nt.handle(), tx).await;
    cam_man.start_dev_providers().await;
    // Spawn the web server
    let api = tokio::spawn(run_api(cam_man.clone()));

    // Poll the API server future until the end of time, ctrl+c, or a message on the shutdown channel
    tokio::select!(
        _ = api => {},
        _ = tokio::signal::ctrl_c() => {},
        _ = runner => {},
        _ = rx.recv() => {},
    );

    Ok(())
}
