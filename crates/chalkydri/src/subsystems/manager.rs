use std::sync::{Arc, Mutex};
use std::thread::Thread;

use gstreamer::{Element, Pipeline};
use tokio::task::JoinSet;
use tokio_util::task::TaskTracker;
use tracing::Level;

#[cfg(feature = "capriltags")]
use super::capriltags::{self, CApriltagsDetector};
#[cfg(feature = "python")]
use super::python::PythonSubsys;
#[cfg(feature = "python")]
use crate::subsystems::python::PythonPreproc;
#[cfg(feature = "capriltags")]
use crate::{cameras::preproc::PreprocWrap, subsystems::capriltags::CapriltagsPreproc};
use crate::{cameras::CamManager, config, error::Error, pose::PoseEstimator, Nt};
use crate::{
    cameras::{mjpeg::MjpegProc, preproc::Preprocessor},
    subsystems::Subsystem,
};

/// The subsystem manager
///
///
#[derive(Clone)]
pub struct SubsysManager {
    pub pose_est: PoseEstimator,

    set: Arc<Mutex<Vec<Thread>>>,
    tt: TaskTracker,

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

            set: Arc::new(Mutex::new(Vec::new())),
            tt: TaskTracker::new(),

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
        let set = self.set.clone();

        //#[cfg(feature = "capriltags")]
        //{
        //    let cam_config = cam_config.clone();
        //    manager.capriltags_preproc.setup_sampler(None).unwrap();
        //    std::thread::spawn(move || {
        //        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        //        rt.block_on(async move {
        //            manager_
        //                .capriltags
        //                .process(Nt.handle(), cam_config, manager_.capriltags_preproc.rx())
        //                .await
        //                .unwrap();
        //        });
        //    });
        //}

        #[cfg(feature = "python")]
        {
            let cam_config = cam_config.clone();
            manager.python_preproc.setup_sampler(None).unwrap();
            let thread = std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                rt.block_on(async move {
                            manager_
                                .python
                                .process(Nt.handle(), cam_config, manager_.python_preproc.rx())
                                .await
                                .unwrap();
                });
            });

            set.lock().unwrap().push(thread.thread().to_owned());
        }
    }

    pub fn link(&self, src: &Element) {
        //#[cfg(feature = "capriltags")]
        //self.capriltags_preproc.link(src.clone());
        #[cfg(feature = "python")]
        self.python_preproc.link(src.clone());
    }

    pub fn unlink(&self, src: &Element) {
        //#[cfg(feature = "capriltags")]
        //self.capriltags_preproc.unlink(src.clone());
        #[cfg(feature = "python")]
        self.python_preproc.unlink(src.clone());
    }
}
