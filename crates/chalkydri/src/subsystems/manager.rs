use gstreamer::{Element, Pipeline};
use tracing::Level;

use crate::{cameras::{mjpeg::MjpegProc, pipeline::Preprocessor}, subsystems::Subsystem};
#[cfg(feature = "capriltags")]
use super::capriltags::{self, CApriltagsDetector};
#[cfg(feature = "python")]
use super::python::PythonSubsys;
use crate::{Nt, cameras::CamManager, config, error::Error, pose::PoseEstimator};

#[derive(Clone)]
pub struct SubsysManager {
    pub pose_est: PoseEstimator,
    
    mjpeg: MjpegProc,
    #[cfg(feature = "capriltags")]
    capriltags: CApriltagsDetector,
    #[cfg(feature = "python")]
    python: PythonSubsys,
}
impl SubsysManager {
    /// Initialize the [`subsystem`](Subsystem) manager
    pub async fn new(pipeline: &Pipeline) -> Result<Self, Error> {
        let span = span!(Level::INFO, "subsys_manager");
        let _enter = span.enter();

        let pose_est = PoseEstimator::new().await?;

        let mjpeg = <MjpegProc as Preprocessor>::new(pipeline);

        #[cfg(feature = "capriltags")]
        let capriltags = CApriltagsDetector::init().await.unwrap();

        #[cfg(feature = "python")]
        let python = PythonSubsys::init().await.unwrap();

        Ok(Self {
            pose_est,

            mjpeg,
            #[cfg(feature = "capriltags")]
            capriltags,
            #[cfg(feature = "python")]
            python,
        })
    }

    /// Spawn subsystems for a camera
    pub async fn start(&self, cam_config: config::Camera, pipeline: &Pipeline, cam: &Element) {
        let manager = self.clone();
        let manager_ = manager.clone();

        manager
            .mjpeg
            .process(manager_, Nt.clone(), cam_config)
            .await
            .unwrap();
    }
}
