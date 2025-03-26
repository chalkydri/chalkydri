use gstreamer::{Buffer, Element, Pipeline};
use tokio::sync::watch;
use tracing::Level;

#[cfg(feature = "capriltags")]
use super::capriltags::{self, CApriltagsDetector};
#[cfg(feature = "python")]
use super::{Subsystem, python::PythonSubsys};
use crate::{Cfg, Nt, cameras::CameraManager, config, error::Error, pose::PoseEstimator};

#[derive(Clone)]
pub struct SubsysManager {
    pub pose_est: PoseEstimator,
    #[cfg(feature = "capriltags")]
    capriltags: CApriltagsDetector,
    #[cfg(feature = "python")]
    python: PythonSubsys,
}
impl SubsysManager {
    pub async fn new() -> Result<Self, Error> {
        let span = span!(Level::INFO, "subsys_manager");
        let _enter = span.enter();

        let pose_est = PoseEstimator::new().await?;
        #[cfg(feature = "capriltags")]
        let capriltags = CApriltagsDetector::init().await.unwrap();
        #[cfg(feature = "python")]
        let python = PythonSubsys::init().await.unwrap();

        Ok(Self {
            pose_est,
            #[cfg(feature = "capriltags")]
            capriltags,
            #[cfg(feature = "python")]
            python,
        })
    }
    pub fn spawn(&self, cam_config: config::Camera, pipeline: &Pipeline, cam: &Element) {
        //if let Some(cameras) = Cfg.read().await.cameras {
        //    for camera in cameras {
        //        let cam_config = camera.clone();
        let manager = self.clone();
        #[cfg(feature = "capriltags")]
        let capriltags_rx = CameraManager::add_subsys::<CApriltagsDetector>(
            pipeline,
            cam,
            cam_config.clone(),
            true,
        );
        #[cfg(feature = "python")]
        let python_rx =
            CameraManager::add_subsys::<PythonSubsys>(pipeline, cam, cam_config.clone(), true);
        std::thread::spawn(|| {
            futures_executor::block_on(async move {
                let manager_ = manager.clone();
                #[cfg(feature = "capriltags")]
                {
                    manager
                        .capriltags
                        .process(manager_, Nt.clone(), cam_config, capriltags_rx)
                        .await
                        .unwrap();
                }

                #[cfg(feature = "python")]
                {
                    manager
                        .python
                        .process(manager_, Nt.clone(), cam_config, python_rx)
                        .await
                        .unwrap();
                }
            })
        });
        //    }
        //}
    }
}
