//!
//! Chalkydri core
//!

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
//mod subsys;
mod config;
mod api;
mod utils;

use std::{error::Error, time::Duration};
use minint::NtConn;
use tokio::runtime::Runtime;

use crate::{api::run_api, utils::gen_team_ip};

pub trait Subsystem<'subsys> {
    fn init() -> Result<Box<Self>, Box<dyn Error>>;
    fn run(&self, rt: Runtime);
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Chalkydri starting up...");

    let roborio_ip = gen_team_ip(4533).unwrap();
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
