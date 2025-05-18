use std::collections::HashMap;

use minint::{NtSubscription, NtTopic};

use crate::{config, Nt};

struct Cam<'p> {
    source: NtTopic<'p, String>,
    streams: NtTopic<'p, Vec<String>>,
    description: NtTopic<'p, String>,
    connected: NtTopic<'p, bool>,
    mode: NtSubscription<'p>,
    modes: NtTopic<'p, Vec<String>>,
}

pub struct CamPublisher<'p> {
    cams: HashMap<String, Cam<'p>>,
}
impl CamPublisher<'_> {
    pub fn new() -> Self {
        Self {
            cams: HashMap::new(),
        }
    }

    pub async fn publish(&mut self, cam_config: config::Camera) {
        let cam = if let Some(cam) = self.cams.get_mut(&cam_config.name) {
            cam
        } else {
            let source = Nt
                .publish::<String>(&format!("/CameraPublisher/{}/source", cam_config.id))
                .await
                .unwrap();
            let streams = Nt
                .publish::<Vec<String>>(&format!("/CameraPublisher/{}/streams", cam_config.id))
                .await
                .unwrap();
            let description = Nt
                .publish::<String>(&format!("/CameraPublisher/{}/description", cam_config.id))
                .await
                .unwrap();
            let connected = Nt
                .publish::<bool>(&format!("/CameraPublisher/{}/connected", cam_config.id))
                .await
                .unwrap();
            let mode = Nt
                .subscribe(&format!("/CameraPublisher/{}/mode", cam_config.id))
                .await
                .unwrap();
            let modes = Nt
                .publish::<Vec<String>>(&format!("/CameraPublisher/{}/modes", cam_config.id))
                .await
                .unwrap();

            let cam = Cam {
                source,
                streams,
                description,
                connected,
                mode,
                modes,
            };

            self.cams.insert(cam_config.name.clone(), cam);

            self.cams.get_mut(&cam_config.name).unwrap()
        };
        debug!("publishing camera {}", cam_config.name);

        let hostname = rustix::system::uname()
            .nodename()
            .to_str()
            .unwrap()
            .to_string();

        cam.source.set("usb:0".to_string()).await.unwrap();
        cam.streams.set(vec![format!(
            "mjpeg:http://{hostname}.local:6942/stream/{}",
            cam_config.id,
        )]).await.unwrap();
        cam.description.set("Chalkydri".to_string()).await.unwrap();
        cam.connected.set(true).await.unwrap();
        cam.modes.set(vec!["On".to_owned(), "Off".to_owned()]).await.unwrap();
    }
}
