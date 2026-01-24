use std::collections::HashMap;

use nt_client::{NewClientOptions, data::Properties, publish::Publisher, subscribe::Subscriber};

use crate::{Nt, config};

struct Cam {
    source: Publisher<String>,
    streams: Publisher<Vec<String>>,
    description: Publisher<String>,
    connected: Publisher<bool>,
    mode: Subscriber,
    modes: Publisher<Vec<String>>,
}

pub struct CamPublisher {
    cams: HashMap<String, Cam>,
}
impl CamPublisher {
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
                .topic(format!("/CameraPublisher/{}/source", cam_config.id))
                .publish::<String>(Default::default())
                .await
                .unwrap();
            let streams = Nt
                .topic(format!("/CameraPublisher/{}/streams", cam_config.id))
                .publish::<Vec<String>>(Default::default())
                .await
                .unwrap();
            let description = Nt
                .topic(format!("/CameraPublisher/{}/description", cam_config.id))
                .publish::<String>(Default::default())
                .await
                .unwrap();
            let connected = Nt
                .topic(format!("/CameraPublisher/{}/connected", cam_config.id))
                .publish::<bool>(Default::default())
                .await
                .unwrap();
            let mode = Nt
                .topic(format!("/CameraPublisher/{}/mode", cam_config.id))
                .subscribe(Default::default())
                .await
                .unwrap();
            let modes = Nt
                .topic(format!("/CameraPublisher/{}/modes", cam_config.id))
                .publish::<Vec<String>>(Default::default())
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
        tracing::debug!("publishing camera {}", cam_config.name);

        let hostname = rustix::system::uname()
            .nodename()
            .to_str()
            .unwrap()
            .to_string();

        cam.source.set("usb:0".to_string()).await.unwrap();
        cam.streams
            .set(vec![format!(
                "mjpeg:http://{hostname}.local:6942/stream/{}",
                cam_config.id,
            )])
            .await
            .unwrap();
        cam.description.set("Chalkydri".to_string()).await.unwrap();
        cam.connected.set(true).await.unwrap();
        cam.modes
            .set(vec!["On".to_owned(), "Off".to_owned()])
            .await
            .unwrap();
    }
}
