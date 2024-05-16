#[macro_use]
extern crate log;
#[macro_use]
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

use std::{error::Error, net::SocketAddr, time::Duration};
use actix_web::{web, App, HttpServer, Responder};
use minint::NtConn;
use tokio::runtime::Runtime;
use utopia::OpenApi;
//use minint::{client::{AsyncClientHandle, config::ClientConfig}, spec::topic::PublishProperties};

pub trait Subsystem<'subsys> {
    fn init() -> Result<Box<Self>, Box<dyn Error>>;
    fn run(&self, rt: Runtime);
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Chalkydri starting up...");

    //let ntcfg = ClientConfig::default();
    /*
    let nt = AsyncClientHandle::start("10.45.33.2".parse().unwrap(), ntcfg, String::from("chalkydri")).await.unwrap();
    nt.publish_topic("/chalkydri/G3A19", , None);
    nt.subscribe(&["/chalkydri"]).await.unwrap();
    get_if_addrs();
    */

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let mut nt = NtConn::new([127, 0, 0, 1], "jdiaudicadsicljd").await.unwrap();
        nt.publish::<i32>("/Chalkydri/Devices/1/CameraCount").unwrap().set(9).unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
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
