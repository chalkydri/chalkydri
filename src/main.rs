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
extern crate wgpu;

#[cfg(feature = "libcamera")]
mod cameras;
mod subsys;
mod config;

//use libcamera::camera_manager::CameraManager;
use std::error::Error;
use if_addrs::get_if_addrs;
use tokio::runtime::Runtime;
use wgpu::{Backends, DeviceDescriptor, PowerPreference, RequestAdapterOptions, InstanceDescriptor};

pub trait Subsystem<'subsys> {
    fn init() -> Result<Box<Self>, Box<dyn Error>>;
    fn run(&self, rt: Runtime);
}

fn main() {
    env_logger::init();

    info!("Chalkydri starting up...");

    get_if_addrs();

    let rt = Runtime::new().unwrap();

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
}
