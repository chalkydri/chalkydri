use gstreamer::{Element, Pipeline};
use tracing::Level;

use crate::subsystems::Subsystem;
#[cfg(feature = "capriltags")]
use super::capriltags::{self, CApriltagsDetector};
#[cfg(feature = "python")]
use super::python::PythonSubsys;
use crate::{Nt, cameras::CameraManager, config, error::Error, pose::PoseEstimator};

#[derive(Clone)]
pub struct SubsysManager {
    pub pose_est: PoseEstimator,
    #[cfg(feature = "capriltags")]
    capriltags: CApriltagsDetector,
    #[cfg(feature = "python")]
    python: PythonSubsys,
}
impl SubsysManager {
    /// Initialize the [`subsystem`](Subsystem) manager
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

    /// Spawn subsystems for a camera
    pub async fn spawn(&self, cam_config: config::Camera, pipeline: &Pipeline, cam: &Element) {
        let manager = self.clone();
        let manager_ = manager.clone();

        #[cfg(feature = "capriltags")]
        {
            let span = span!(Level::INFO, "capriltags_subsys", camera = cam_config.name);
            let capriltags_rx = CameraManager::add_subsys::<CApriltagsDetector>(
                pipeline,
                cam,
                cam_config.clone(),
                true,
            );
            manager
                .capriltags
                .process(manager_, Nt.clone(), cam_config, capriltags_rx)
                .await
                .unwrap();
        }

        #[cfg(feature = "python")]
        {
            let python_rx =
                CameraManager::add_subsys::<PythonSubsys>(pipeline, cam, cam_config.clone(), true);
            manager
                .python
                .process(manager_, Nt.clone(), cam_config, python_rx)
                .await
                .unwrap();
        }
    }
}
