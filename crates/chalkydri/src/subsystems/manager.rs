use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread::Thread;

use chalkydri_core::subsystems::SubsystemCtrl;
use gstreamer::Task;
use gstreamer::{Element, Pipeline};
use tokio::sync::mpsc;
use tokio_util::task::TaskTracker;
use tracing::Level;

#[cfg(feature = "python")]
use crate::cameras::preproc::PreprocWrap;
use crate::subsystems::SubsysRunner;
use crate::{Nt, config};
use chalkydri_core::prelude::*;
#[cfg(feature = "capriltags")]
use chalkydri_subsys_capriltags::{self as capriltags, CApriltagsDetector, CapriltagsPreproc};
#[cfg(feature = "python")]
use chalkydri_subsys_python::{PythonPreproc, PythonSubsys};

/// The subsystem manager orchestrates all the subsystems for a pipeline
#[derive(Clone)]
pub struct SubsysManager {
    set: Arc<Mutex<Vec<mpsc::Sender<SubsystemCtrl>>>>,
    tt: TaskTracker,

    #[cfg(feature = "capriltags")]
    capriltags: CApriltagsDetector,
    #[cfg(feature = "capriltags")]
    capriltags_preproc: Arc<PreprocWrap<CapriltagsPreproc>>,
    #[cfg(feature = "python")]
    python: SubsysRunner<PythonPreproc, PythonSubsys>,
}
impl SubsysManager {
    /// Initialize the [`subsystem`](Subsystem) manager
    pub async fn new(
        pipeline: &Pipeline,
        cam_config: config::Camera,
        cam: &Element,
    ) -> Result<Self, Error> {
        let span = span!(Level::INFO, "subsys_manager");
        let _enter = span.enter();

        //#[cfg(feature = "capriltags")]
        //let capriltags = CApriltagsDetector::init().await.unwrap();
        //#[cfg(feature = "capriltags")]
        //let capriltags_preproc = Arc::new(PreprocWrap::new(pipeline));

        let tt = TaskTracker::new();

        #[cfg(feature = "python")]
        let python = SubsysRunner::<PythonPreproc, PythonSubsys>::init(
            pipeline.clone(),
            cam_config.clone(),
            cam.clone(),
            tt.clone(),
        )
        .await;

        Ok(Self {
            set: Arc::new(Mutex::new(Vec::new())),
            tt: TaskTracker::new(),

            #[cfg(feature = "capriltags")]
            capriltags,
            #[cfg(feature = "capriltags")]
            capriltags_preproc,
            #[cfg(feature = "python")]
            python,
        })
    }

    /// Spawn subsystems for a camera
    pub async fn start(&mut self, cam_config: config::Camera, pipeline: &Pipeline, cam: &Element) {
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

            trace!("initializing python subsystem");

            self.python
                .start(cam_config.clone(), cam.clone(), self.tt.clone())
                .await;

            trace!("adding python jh to join set");
            self.set.lock().unwrap().push(self.python.tx.clone());
        }
    }

    pub async fn stop(&mut self) {
        trace!("sending stop msgs");
        while let Some(tx) = self.set.lock().unwrap().pop() {
            tx.send(SubsystemCtrl::Stop).await.unwrap();
        }
    }

    pub fn link(&self, src: &Element) {
        //#[cfg(feature = "capriltags")]
        //self.capriltags_preproc.link(src.clone());
        //#[cfg(feature = "python")]
        //self.python.preproc.link(src.clone());
    }

    pub fn unlink(&self, src: &Element) {
        //#[cfg(feature = "capriltags")]
        //self.capriltags_preproc.unlink(src.clone());
        //#[cfg(feature = "python")]
        //self.python.preproc.unlink(src.clone());
    }

    pub async fn run(&self) {
        trace!("waiting");
        self.tt.wait().await;
        trace!("done");
    }
}
