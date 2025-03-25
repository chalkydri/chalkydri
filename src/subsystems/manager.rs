use gstreamer::Buffer;
use tokio::sync::watch;
use tracing::Level;

use crate::{config, error::Error, pose::PoseEstimator, Cfg, Nt};
use super::{capriltags::{self, CApriltagsDetector}, Subsystem};

#[derive(Clone)]
pub struct SubsysManager {
    pub pose_est: PoseEstimator,
    capriltags: CApriltagsDetector,
}
impl SubsysManager {
    pub async fn new() -> Result<Self, Error> {
        let span = span!(Level::INFO, "subsys_manager");
        let _enter = span.enter();

        let pose_est = PoseEstimator::new().await?;
        let capriltags = CApriltagsDetector::init().await.unwrap();

        Ok(Self {
            pose_est,
            capriltags,
        })
    }
    pub async fn spawn(&self, cam_config: config::Camera, rx: watch::Receiver<Option<Vec<u8>>>) {
        //if let Some(cameras) = Cfg.read().await.cameras {
        //    for camera in cameras {
        //        let cam_config = camera.clone();
        let manager = self.clone();
        tokio::spawn(async move {
            let manager_ = manager.clone();
            manager.capriltags.process(manager_, Nt.clone(), cam_config, rx).await.unwrap();
        });
        //    }
        //}
    }
}
