//!
//! Chalkydri core
//!

// Unsafe code is NOT allowed in Chalkydri core.
// If unsafe code is required, it should be part of a different crate.
#![forbid(unsafe_code)]
#![allow(unreachable_code)]

#[macro_use]
extern crate log;
//extern crate actix_web;
extern crate env_logger;
extern crate minint;
#[cfg(feature = "mjpeg")]
extern crate mozjpeg;
//extern crate ril;
extern crate tokio;
//extern crate utoipa as utopia;
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
//extern crate nokhwa;

//mod api;
mod cameras;
mod config;
mod subsys;
mod utils;
//mod logger;
mod subsystem;
mod calibration;

//use api::run_api;
use cameras::load_cameras;
use mimalloc::MiMalloc;
use minint::NtConn;
#[cfg(feature = "rerun_web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
use re_ws_comms::RerunServerPort;
use rerun::{Image, MemoryLimit, Text, TextLog};
use std::{error::Error, fmt::Debug, marker::PhantomData, sync::Arc, time::Duration};
#[cfg(feature = "capriltags")]
use subsys::capriltags::CApriltagsDetector;
use tokio::sync::watch;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use crate::utils::gen_team_ip;

use subsystem::Subsystem;

#[tokio::main(worker_threads = 16)]
async fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "rerun_web_viewer")]
    let rr = rerun::RecordingStreamBuilder::new("chalkydri")
        .serve_web(
            "0.0.0.0",
            WebViewerServerPort(8080),
            RerunServerPort(6969),
            MemoryLimit::from_bytes(10_000_000),
            true,
        )
        .unwrap();

    // Initialize logger
    //env_logger::init();
    rerun::Logger::new(rr.clone())
        .with_path_prefix("logs/handler")
        .with_filter(rerun::default_log_filter())
        .init()?;

    //// Create a new SystemRunner for Actix
    //let sys = System::with_tokio_rt(|| Runtime::new().unwrap());

    info!("Chalkydri starting up...");

    let roborio_ip = gen_team_ip(4533).expect("failed to generate team ip");
    // Generate a random device id
    let dev_id = fastrand::u32(..);

    // Attempt to connect to the NT server, retrying until successful

    //let nt: NtConn;

    //let mut retry = false;

    //loop {
    //    match NtConn::new(roborio_ip, format!("chalkydri{dev_id}")).await {
    //        Ok(conn) => {
    //            nt = conn;
    //            break;
    //        }
    //        Err(err) => {
    //            if !retry {
    //                error!("Error connecting to NT server: {err:?}");
    //                retry = true;
    //            }
    //            tokio::time::sleep(Duration::from_millis(5)).await;
    //        }
    //    }
    //}

    //info!("Connected to NT server at {roborio_ip:?} successfully!");

    let (tx, mut rx) = watch::channel::<Arc<Vec<u8>>>(Arc::new(Vec::new()));

    let rr_ = rr.clone();
    std::thread::spawn(move || {
        let tx = tx.clone();
        load_cameras(tx, rr_).unwrap();
    });

    // apriltag C library subsystem
    {
        let mut at = CApriltagsDetector::init(()).await.unwrap();

        //let nt = nt.clone();
        loop {
            // Wait for a new image from the camera
            if rx.changed().await.is_ok() {
                let buf = rx.borrow_and_update();
                //rr.log("/working", &TextLog::new("ITS WORKING")).unwrap();
                //rr.log("/images", &Image::from_rgb24(buf.clone().to_vec(), [1280, 720]))
                //    .unwrap();

                // Send the buffer to AprilTag detector
                let buf_ = buf.clone();
                drop(buf);
                let pose = at.process(buf_, rr.clone()).unwrap();
                info!("{pose:?}");

                //let mut translation = nt
                //    .publish::<Vec<f64>>(&format!("/chalkydri/robot_pose/translation"))
                //    .await
                //    .unwrap();
                //let mut rotation = nt
                //    .publish::<Vec<f64>>(&format!("/chalkydri/robot_pose/rotation"))
                //    .await
                //    .unwrap();

                //let (t, r) = pose;

                //translation.set(t.clone()).await.unwrap();
                //rotation.set(r.clone()).await.unwrap();
            }
        }
    }

    // Have to let NT topics get dropped before calling nt.stop()
    {
        //run_api(nt.clone()).await;
    }

    // Shut down NT connection
    //nt.stop();

    Ok(())
}
