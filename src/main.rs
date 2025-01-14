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

#[cfg(feature = "libcamera")]
mod cameras;
//mod api;
mod config;
mod subsys;
mod utils;
//mod logger;

use actix::prelude::*;
use cameras::load_cameras;
use minint::NtConn;
use std::{error::Error, fmt::Debug, marker::PhantomData, sync::Arc, time::Duration};
use subsys::capriltags::CApriltagsDetector;

use crate::utils::gen_team_ip;

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
pub(crate) trait Subsystem<'fr>: Sized {
    /// The actual frame processing [Actor]
    ///
    /// May be `Self`
    type Processor: Actor + Handler<ProcessFrame<Self::Output, Self::Error>>;
    /// The subsystem's configuration type
    type Config;
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    ///
    /// This should initialize the subsystem actor, but not start it.
    async fn init(cfg: Self::Config) -> Result<Addr<Self::Processor>, Self::Error>;
}

/// Actix message for sending a frame to a subsystem for processing
pub(crate) struct ProcessFrame<R, E>
where
    R: Send + 'static,
    E: Debug + Send + 'static,
{
    buf: Arc<Vec<u8>>,
    _marker: PhantomData<(R, E)>,
}
impl<R: Send + 'static, E: Debug + Send + 'static> Message for ProcessFrame<R, E> {
    type Result = Result<R, E>;
}

#[actix::main(worker_threads = 12)]
async fn main() -> Result<(), Box<dyn Error>> {
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

    {
        let tx = tx.clone();
    tokio::task::spawn_blocking(move || {
        load_cameras(tx).unwrap();
    });
    }
    println!("skibidi sigma");

    //let apriltags_subsys = Apriltags::init().await.unwrap();
    //let ml_subsys = MlSubsys::init().await?;

    //let apriltags = apriltags_subsys.run(ApriltagsConfig { workers: 4 }).await;
    //let ml = ml_subsys.run(MlSubsysCfg { model_path: String::from("test.tflite") }).await;

        let mut at = CApriltagsDetector::new();
            
        loop {
            println!("skibidi sigma");
            let buf = rx.recv().unwrap();
            let buf = buf.clone();

            at.detect(buf);
            std::thread::sleep(Duration::from_millis(100));
            /*
            at.send(ProcessFrame::<(), _> {
                buf: buf.into(),
                _marker: PhantomData,
            }).await.unwrap();
            */
        }

    // Have to let NT topics get dropped before calling nt.stop()
    {
        //run_api(nt.clone()).await;
    }

    // Shut down NT connection
    nt.stop();

    Ok(())
}
