//mod kinda_just_thrown_together_last_minute_and_crapped_the_bed_format;

use cu29::bundle_resources;
use nt_client::{Client, ClientHandle, NewClientOptions, data::Properties};
use tokio::runtime::Runtime;

pub struct NetworkTables {
    rt: tokio::runtime::Runtime,
    nt: nt_client::Client,
}
impl NetworkTables {
    pub fn new() -> Self {
        let rt = Runtime::new().unwrap();
        let nt = Client::new(NewClientOptions::default());

        Self { rt, nt }
    }
    pub fn run(&self, mut fut: impl AsyncFnMut(&ClientHandle)) {
        let handle = self.nt.handle();
        self.rt.block_on(async move {
            fut(handle).await;
        });
    }
    fn setup(client: &Client) {
        let photon_root = client.topic("/photonvision");

        tokio::spawn(async move {
            let camera_root = photon_root.child("/test");
            let driver_mode = camera_root
                .child("/driverMode")
                .publish::<bool>(Default::default())
                .await
                .unwrap();
            let fps_limit = camera_root
                .child("/fpsLimit")
                .publish::<i32>(Default::default())
                .await
                .unwrap();
            let pipeline_index_state = camera_root
                .child("/pipelineIndexState")
                .publish::<i32>(Default::default())
                .await
                .unwrap();
            let heartbeat = camera_root
                .child("/heartbeat")
                .publish::<i32>(Default::default())
                .await
                .unwrap();
            let led_mode_state = camera_root
                .child("/ledModeState")
                .publish::<i32>(Default::default())
                .await
                .unwrap();
            let version = camera_root
                .child("/version")
                .publish::<String>(Default::default())
                .await
                .unwrap();
        });
        client.topics(
            [
                "/FMSInfo/FMSControlData",
                "/FMSInfo/EventName",
                "/FMSInfo/MatchType",
                "/FMSInfo/MatchNumber",
                "/FMSInfo/ReplayNumber",
                "/FMSInfo/isRedAlliance",
                "/FMSInfo/StationNumber",
                "/photonvision/apriltag_field_layout",
            ]
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>(),
        );
    }
}

pub struct PhonyVisionBundle;
bundle_resources!(PhonyVisionBundle: PhonyVision);

pub struct PhonyVision {
    coprocessors: nt_client::subscribe::Subscriber,
}
