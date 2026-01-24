use std::sync::{Arc, Mutex};

use gstreamer::{Element, Pipeline};
use tokio::task::JoinSet;
use tokio_util::task::TaskTracker;
use tracing::Level;

#[cfg(feature = "python")]
use crate::subsystems::python::PythonPreproc;
#[cfg(feature = "capriltags")]
use crate::{cameras::pipeline::PreprocWrap, subsystems::capriltags::CapriltagsPreproc};
use crate::{cameras::{mjpeg::MjpegProc, pipeline::Preprocessor}, subsystems::Subsystem};
#[cfg(feature = "capriltags")]
use super::capriltags::{self, CApriltagsDetector};
#[cfg(feature = "python")]
use super::python::PythonSubsys;
use crate::{Nt, cameras::CamManager, config, error::Error, pose::PoseEstimator};

#[derive(Clone)]
pub struct SubsysManager {
    pub pose_est: PoseEstimator,

    set: TaskTracker,
    
    #[cfg(feature = "capriltags")]
    capriltags: CApriltagsDetector,
    #[cfg(feature = "capriltags")]
    capriltags_preproc: Arc<PreprocWrap<CapriltagsPreproc>>,

    #[cfg(feature = "python")]
    python: PythonSubsys,
    #[cfg(feature = "python")]
    python_preproc: Arc<PreprocWrap<PythonPreproc>>,
}
impl SubsysManager {
    /// Initialize the [`subsystem`](Subsystem) manager
    pub async fn new(pipeline: &Pipeline) -> Result<Self, Error> {
        let span = span!(Level::INFO, "subsys_manager");
        let _enter = span.enter();

        let pose_est = PoseEstimator::new().await?;

        #[cfg(feature = "capriltags")]
        let capriltags = CApriltagsDetector::init().await.unwrap();
        #[cfg(feature = "capriltags")]
        let capriltags_preproc = Arc::new(PreprocWrap::new(pipeline));

        #[cfg(feature = "python")]
        let python = PythonSubsys::init().await.unwrap();
        #[cfg(feature = "python")]
        let python_preproc = Arc::new(PreprocWrap::new(pipeline));

        Ok(Self {
            pose_est,

            set: TaskTracker::new(),

            #[cfg(feature = "capriltags")]
            capriltags,
            #[cfg(feature = "capriltags")]
            capriltags_preproc,

            #[cfg(feature = "python")]
            python,
            #[cfg(feature = "python")]
            python_preproc,
        })
    }

    /// Spawn subsystems for a camera
    pub async fn start(&self, cam_config: config::Camera, pipeline: &Pipeline, cam: &Element) {
        let manager = self.clone();
        let manager_ = manager.clone();

        #[cfg(feature = "capriltags")]
        self.set.spawn(async move {
            println!("a");
            manager.capriltags_preproc.setup_sampler(None).unwrap();
            manager
                .capriltags
                .process(Nt.handle(), cam_config, manager_.capriltags_preproc.rx())
                .await
                .unwrap();
        });
    }
}
