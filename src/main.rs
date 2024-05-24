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
//extern crate minint;
#[macro_use]
extern crate actix_web;
extern crate utoipa as utopia;
#[macro_use]
extern crate serde;

#[cfg(feature = "libcamera")]
mod cameras;
//mod subsys;
mod config;
mod api;
mod utils;

use std::{error::Error, time::Duration};
use actix_web::{App, HttpServer};
use minint::NtConn;
use tokio::runtime::Runtime;

use crate::utils::gen_team_ip;
//use minint::{client::{AsyncClientHandle, config::ClientConfig}, spec::topic::PublishProperties};

pub trait Subsystem<'subsys> {
    fn init() -> Result<Box<Self>, Box<dyn Error>>;
    fn run(&self, rt: Runtime);
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Chalkydri starting up...");

    let roborio_ip = gen_team_ip(4533).unwrap();

    let dev_id = fastrand::u32(..);

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let nt: NtConn;

        // Attempt to connect to the NT server, retrying until successful

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

        {
            let mut camera_count = nt.publish::<i32>(format!("/Chalkydri/Devices/{dev_id}/CameraCount")).await.unwrap();
            //let mut camera_count2 = nt.publish::<i32>(format!("/Chalkydri/Devices/{dev_id}/CameraCount2")).await.unwrap();
            //drop(camera_count2);

            camera_count.set(9).await.unwrap();
        }

        nt.stop();
        
        HttpServer::new(|| {
            App::new().service(api::info).service(api::configurations)
        })
        .bind(("0.0.0.0", 6942)).unwrap()
        .run()
        .await
        .unwrap();
    });

    /*
    rt.block_on(async {
        let ins = wgpu::Instance::new(InstanceDescriptor::default());
        let adapter = ins
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .unwrap();

        let (dev, queue) = adapter
            .request_device(&DeviceDescriptor::default(), None)
            .await
            .unwrap();

        info!("acquired gpu: {}", adapter.get_info().name);

        //let cam = ;

        fn start<'a, S: Subsystem<'a>>(rt: Runtime) {
            info!("initializing...");
            let s = S::init().unwrap();
            info!("running...");
            s.run(rt);
        }

        //start::<subsys::ml::MlSubsys>(rt);

        info!("Connecting to NT...");
        info!("Connected to NT at {ip}", ip = "10.45.33.2");

        info!("Starting ML...");
        info!("Ready for inference");

        info!("Chalkydri ready");
    });
    */

    Ok(())
}
