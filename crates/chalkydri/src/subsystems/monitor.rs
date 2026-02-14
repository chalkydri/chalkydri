use cu_sensor_payloads::CuImage;
use cu29::prelude::*;
use image::Luma;
use rerun::{
    MemoryLimit, PlaybackBehavior, RecordingStream, RecordingStreamBuilder, ServerOptions,
    web_viewer::WebViewerConfig,
};

pub struct MonitorBundle;
bundle_resources!(MonitorBundle: Monitor);

impl ResourceBundle for MonitorBundle {
    fn build(
        bundle: BundleContext<Self>,
        config: Option<&ComponentConfig>,
        manager: &mut ResourceManager,
    ) -> CuResult<()> {
        let monitor_key = bundle.key(MonitorBundleId::Monitor);

        manager.add_owned(monitor_key, MonitorResource::new())?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct MonitorResource {
    stream: RecordingStream,
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

pub struct Resources<'r> {
    pub monitor: Borrowed<'r, MonitorResource>,
}
impl<'r> ResourceBindings<'r> for Resources<'r> {
    type Binding = MonitorBundleId;
    fn from_bindings(
        manager: &'r mut ResourceManager,
        mapping: Option<&ResourceBindingMap<Self::Binding>>,
    ) -> CuResult<Self> {
        let key = mapping
            .expect("comm binding")
            .get(Self::Binding::Monitor)
            .expect("comm")
            .typed();
        Ok(Self {
            monitor: manager.borrow(key)?,
        })
    }
}

pub struct Monitor {
    cam_id: u8,
    monitor: MonitorResource,
}
impl Freezable for Monitor {}
impl CuSinkTask for Monitor {
    type Input<'m> = input_msg!((CuImage<Vec<u8>>, CuDuration));
    type Resources<'r> = Resources<'r>;

    fn new(config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let cam_id = config
            .expect("config must be present")
            .get::<u8>("cam_id")
            .expect("cam_id must be set");

        let monitor = resources.monitor.0.clone();

        Ok(Self { cam_id, monitor })
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        if let Some(payload) = input.payload() {
            self.monitor
                .stream
                .set_time_sequence("time", clock.now().as_micros() as i64);

            let img = payload.0.as_image_buffer::<Luma<u8>>().unwrap();

            self.monitor
                .stream
                .log(
                    format!("cam{}/image", self.cam_id),
                    &rerun::Image::from_l8(img.to_vec(), [img.width(), img.height()]),
                )
                .unwrap();
        }

        Ok(())
    }
}
