//!
//! Chalkydri core
//!

// Unsafe code is NOT allowed in Chalkydri core.
// If unsafe code is required, it should be part of a different crate.
#![forbid(unsafe_code)]

#[macro_use]
extern crate log;
extern crate actix;
extern crate actix_web;
extern crate env_logger;
extern crate fast_image_resize;
#[cfg(feature = "libcamera")]
extern crate libcamera;
extern crate minint;
#[cfg(feature = "mjpeg")]
extern crate mozjpeg;
extern crate ril;
extern crate tokio;
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

//mod api;
#[cfg(feature = "libcamera")]
mod cameras;
mod config;
mod subsys;
mod utils;
//mod logger;
mod subsystem;

use actix::prelude::*;
//use api::run_api;
use cameras::load_cameras;
use minint::NtConn;
use re_web_viewer_server::WebViewerServerPort;
use re_ws_comms::RerunServerPort;
use rerun::{Image, MemoryLimit};
use std::{error::Error, fmt::Debug, marker::PhantomData, sync::Arc, time::Duration};
use subsys::capriltags::CApriltagsDetector;

use crate::utils::gen_team_ip;

use subsystem::{ProcessFrame, Subsystem};

#[actix::main(worker_threads = 12)]
async fn main() -> Result<(), Box<dyn Error>> {
    let rr = rerun::RecordingStreamBuilder::new("chalkydri")
        .serve_web(
            "0.0.0.0",
            WebViewerServerPort(8080),
            RerunServerPort(6969),
            MemoryLimit::from_bytes(1000000),
            false,
        )
        .unwrap();
    // Initialize logger
    env_logger::init();

    //// Create a new SystemRunner for Actix
    //let sys = System::with_tokio_rt(|| Runtime::new().unwrap());

    info!("Chalkydri starting up...");

    let roborio_ip = gen_team_ip(4533).expect("failed to generate team ip");
    // Generate a random device id
    let dev_id = fastrand::u32(..);

    // Attempt to connect to the NT server, retrying until successful

    let nt: NtConn;

    let mut retry = false;

    loop {
        match NtConn::new(roborio_ip, format!("chalkydri{dev_id}")).await {
            Ok(conn) => {
                nt = conn;
                break;
            }
            Err(err) => {
                if !retry {
                    error!("Error connecting to NT server: {err:?}");
                    retry = true;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    info!("Connected to NT server at {roborio_ip:?} successfully!");

    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();

    std::thread::spawn(move || {
        let tx = tx.clone();
        load_cameras(tx).unwrap();
    });

    // apriltag C library subsystem
    {
        let mut at = CApriltagsDetector::init(()).await.unwrap();

        let nt = nt.clone();
            loop {
                // Wait for a new image from the camera
                let buf = rx.recv().unwrap();
                rr.log("images", Image::from_pixel_format([1920, 1080], rerun::PixelFormat::R, bytes));

                // Send the buffer to AprilTag detector
                let poses = at
                    .send(ProcessFrame::<Vec<(Vec<f64>, Vec<f64>)>, _> {
                        buf: buf.into(),
                        _marker: PhantomData,
                    })
                    .await
                    .unwrap()
                    .unwrap();

                    for (i, pose) in poses.into_iter().enumerate() {
                        let mut translation = nt
                            .publish::<Vec<f64>>(&format!(
                                "/chalkydri/apriltags/poses/{i}/translation"
                            ))
                            .await
                            .unwrap();
                        let mut rotation = nt
                            .publish::<Vec<f64>>(&format!(
                                "/chalkydri/apriltags/poses/{i}/rotation"
                            ))
                            .await
                            .unwrap();
                        let (t, r) = pose;
                        translation.set(t.clone()).await.unwrap();
                        rotation.set(r.clone()).await.unwrap();
                    }
            }
    }

    // Have to let NT topics get dropped before calling nt.stop()
    {
        //run_api(nt.clone()).await;
    }

    // Shut down NT connection
    nt.stop();

    Ok(())
}
