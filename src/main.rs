//!
//! Chalkydri core
//!

#![feature(duration_millis_float)]
// Unsafe code is NOT allowed in Chalkydri core.
// If unsafe code is required, it should be part of a different crate.
//#![forbid(unsafe_code)]
#![allow(unreachable_code)]

#[macro_use]
extern crate tracing;
#[cfg(feature = "web")]
extern crate actix_web;
extern crate env_logger;
extern crate minint;
extern crate tokio;
#[cfg(feature = "web")]
extern crate utoipa as utopia;
#[macro_use]
extern crate serde;
#[cfg(feature = "capriltags")]
extern crate apriltag;
#[cfg(feature = "apriltags")]
extern crate chalkydri_apriltags;
#[cfg(feature = "python")]
extern crate pyo3;
#[cfg(feature = "ml")]
extern crate tfledge;

#[cfg(feature = "web")]
mod api;
mod calibration;
mod cameras;
mod config;
mod error;
//mod logger;
mod pose;
mod subsystems;
mod utils;

#[cfg(feature = "web")]
use api::run_api;
use cameras::CameraManager;
use config::Config;
use mimalloc::MiMalloc;
use minint::NtConn;
use once_cell::sync::Lazy;
#[cfg(feature = "rerun")]
use re_sdk::{MemoryLimit, RecordingStream};
#[cfg(feature = "rerun_web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "rerun")]
use re_ws_comms::RerunServerPort;
use std::{error::Error, net::Ipv4Addr, path::Path, sync::Arc};
use tokio::sync::{RwLock, mpsc};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

// mimalloc is a very good general purpose allocator
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use utils::gen_team_ip;

use subsystems::Subsystem;

#[allow(non_upper_case_globals)]
static Cfg: Lazy<Arc<RwLock<Config>>> = Lazy::new(|| {
    let mut path = Path::new("/boot/chalkydri.toml");
    if !path.exists() {
        path = Path::new("/etc/chalkydri.toml");
        if !path.exists() {
            path = Path::new("./chalkydri.toml");
        }
    }

    Arc::new(RwLock::new(Config::load(path).unwrap()))
});

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

#[allow(non_upper_case_globals)]
static Nt: Lazy<NtConn> = Lazy::new(|| {
    futures_executor::block_on(async {
        // Come up with an IP address for the roboRIO based on the team number or specified IP
        let roborio_ip = {
            let Config {
                ntables_ip,
                team_number,
                ..
            } = &*Cfg.read().await;

            ntables_ip
                .clone()
                .map(|s| {
                    s.parse::<Ipv4Addr>()
                        .expect("failed to parse ip address")
                        .octets()
                })
                .unwrap_or_else(|| gen_team_ip(*team_number).expect("failed to generate team ip"))
        };

        // Get the device's name or generate one if not set
        let dev_name = if let Some(dev_name) = (*Cfg.read().await).device_name.clone() {
            dev_name
        } else {
            warn!("device name not set! generating one...");

            // Generate & save it
            let dev_name = String::from("chalkydri");
            (*Cfg.write().await).device_name = Some(dev_name.clone());

            dev_name
        };

        let nt: NtConn;

        match NtConn::new(Ipv4Addr::from(roborio_ip).to_string(), dev_name.clone()).await {
            Ok(conn) => {
                nt = conn;
            }
            Err(err) => {
                panic!("Error connecting to NT server: {err:?}");
            }
        }

        info!("Connected to NT server at {roborio_ip:?} successfully!");

        nt
    })
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

    let filter = EnvFilter::from_default_env();
    let layer = tracing_subscriber::fmt::layer().with_filter(filter);
    tracing_subscriber::registry().with(layer).init();

    info!("starting up...");

    // Disable BS kernel modules
    let _ = rustix::system::delete_module(c"rpivid_hevc", 0);
    let _ = rustix::system::delete_module(c"pisp_be", 0);

    gstreamer::init().unwrap();
    debug!("initialized gstreamer");

    let (tx, mut rx) = mpsc::channel::<()>(1);
    let cam_man = CameraManager::new(Nt.clone(), tx).await;
    let api = tokio::spawn(run_api(cam_man.clone()));

    tokio::select!(
        _ = api => {},
        _ = tokio::signal::ctrl_c() => {},
        _ = rx.recv() => {},
    );

    Ok(())
}
