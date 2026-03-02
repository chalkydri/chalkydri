use cu_sensor_payloads::CuImage;
use cu29::prelude::*;
use image::{DynamicImage, GenericImage, ImageBuffer, Luma};
use rerun::{
    MemoryLimit, PlaybackBehavior, RecordingStream, RecordingStreamBuilder, ServerOptions,
    web_viewer::WebViewerConfig,
};
use std::sync::{Arc, LazyLock};
use turbojpeg::{Image, Subsamp};

pub static MONITOR: LazyLock<Arc<MonitorResource>> = LazyLock::new(|| {
    let mon = MonitorResource::new();
    Arc::new(mon)
});

#[derive(Clone)]
pub struct MonitorResource {
    pub(crate) stream: RecordingStream,
}
impl MonitorResource {
    pub fn new() -> Self {
        let stream = RecordingStreamBuilder::new("chalkydri")
            .serve_grpc_opts(
                "0.0.0.0",
                6767,
                ServerOptions {
                    playback_behavior: PlaybackBehavior::NewestFirst,
                    memory_limit: MemoryLimit::from_fraction_of_total(0.25),
                },
            )
            .unwrap();

        //let mut web_config = WebViewerConfig::default();
        //web_config.connect_to = vec!["rerun+http://localhost/proxy".to_owned()];
        //web_config.open_browser = false;
        let ip_addrs = sysinfo::Networks::new_with_refreshed_list()
            .iter()
            .map(|net| {
                net.1
                    .ip_networks()
                    .into_iter()
                    .map(|ip_net| ip_net.addr)
                    .filter(|ip_addr| !ip_addr.is_loopback() && ip_addr.is_ipv4())
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<Vec<_>>();
        //web_config.bind_ip = "0.0.0.0".to_owned();
        //let wv_server = web_config.host_web_viewer().unwrap();
        for ip_addr in ip_addrs {
            println!("rerun+http://{ip_addr}:6767/proxy");
        }
        //wv_server.detach();

        Self { stream }
    }
}

#[derive(Clone, Reflect)]
#[reflect(from_reflect = false)]
pub struct Monitor;
impl Freezable for Monitor {}
impl CuSinkTask for Monitor {
    type Input<'m> = input_msg!((CuImage<Vec<u8>>, CuDuration));
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self)
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        if let Some(payload) = input.payload() {
            MONITOR
                .stream
                .set_time("/cam/time", std::time::SystemTime::now());

            let img = payload.0.as_image_buffer::<Luma<u8>>().unwrap();

            let img: ImageBuffer<Luma<u8>, Vec<u8>> =
                ImageBuffer::from_raw(img.width(), img.height(), img.as_raw().to_vec()).unwrap();
            let buf = turbojpeg::compress_image(&img, 20, Subsamp::Gray).unwrap();

            MONITOR
                .stream
                .log("/cam/image", &rerun::EncodedImage::new(buf.to_vec()))
                .unwrap();
        }

        Ok(())
    }
}
