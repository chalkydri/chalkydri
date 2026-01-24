//!
//! Subsystem for the official AprilTags C library
//!

// TODO: implement this
// There's actually already a decent Rust binding we can use.
// There's an example here: <https://github.com/jerry73204/apriltag-rust/blob/master/apriltag/examples/detector.rs>
//
// <https://www.chiefdelphi.com/t/frc-blog-technology-updates-past-present-future-and-beyond-apriltags-and-new-radio/440931>
// According to this post on CD, we're doing the 36h11 tag family now.

use std::collections::HashMap;
use std::sync::Arc;

use apriltag::{Detector, Family, Image, TagParams};
use camera_intrinsic_model::{GenericModel, OpenCVModel5};
use chalkydri_core::subsystems::SubsysProcessor;
use gstreamer::ElementFactory;
use gstreamer::prelude::GstBinExtManual;
use gstreamer::{Buffer, Caps, Element};
use nalgebra::{self as na, MatrixView3, MatrixView3x1};
#[cfg(feature = "rerun")]
use re_sdk::external::re_types_core;
#[cfg(feature = "rerun")]
use re_types::{
    archetypes::{Boxes2D, Points2D},
    components::{PinholeProjection, PoseRotationQuat, Position2D, ViewCoordinates},
};
use sophus_autodiff::linalg::{MatF64, VecF64};
use sophus_lie::{Isometry3F64, Rotation3F64};
use std::time::Instant;
use tokio::sync::{Mutex, watch};

use crate::cameras::preproc::Preprocessor;
use crate::error::Error;
use crate::pose::PoseEstimator;
use crate::{Cfg, Nt};
use crate::{
    config, subsystems::Subsystem, subsystems::calibration::CalibratedModel,
    subsystems::frame_proc_loop,
};

const TAG_SIZE: f64 = 0.1651;

/// An AprilTags subsystem using the official C library
#[derive(Clone)]
pub struct CApriltagsDetector {
    det: Arc<Mutex<apriltag::Detector>>,
}
impl Subsystem for CApriltagsDetector {
    const NAME: &'static str = "capriltags";

    type Config = config::CAprilTagsSubsys;
    type Output = ();
    type Preproc = CapriltagsPreproc;
    type Proc = Self;
    type Error = Box<dyn std::error::Error + Send>;

    async fn init(nt: &nt_client::ClientHandle, cam_config: config::Camera) -> Result<Self, <Self::Proc as chalkydri_core::subsystems::SubsysProcessor>::Error> {
        let det = Detector::builder()
            .add_family_bits(Family::tag_36h11(), 3)
            .build()
            .unwrap();

        Ok(Self {
            det: Arc::new(Mutex::new(det)),
        })
    }
}
impl SubsysProcessor for CApriltagsDetector {
    type Subsys = Self;
    type Output = ();
    type Error = Box<dyn std::error::Error + Send>;

    fn stop(&mut self) {
        
    }

    async fn process(
            &self,
            subsys: Self::Subsys,
            nt: &nt_client::ClientHandle,
            cam_config: config::Camera,
            frame: Arc<Vec<u8>>,
        ) -> Result<Self::Output, Self::Error> {
        let model = CalibratedModel::new(cam_config.calib.unwrap());
        let cam_name = cam_config.name.clone();

        // Publish NT topics we'll use
        let mut delay = nt
            .topic(format!("/chalkydri/cameras/{cam_name}/delay"))
            .publish::<f64>(Default::default())
            .await
            .unwrap();
        let mut tag_detected = nt
            .topic(format!("/chalkydri/cameras/{cam_name}/tag_detected"))
            .publish::<bool>(Default::default())
            .await
            .unwrap();

        debug!("running frame processing loop...");
        let det = self.det.clone();

        frame_proc_loop::<Self::Preproc, _>(rx, async move |buf| {
            let proc_st_time = Instant::now();

            debug!("loading image...");
            let settings = cam_config.settings.clone().unwrap();
            match unsafe {
                Image::from_luma8(
                    settings.width as _,
                    settings.height as _,
                    buf.as_ptr() as *mut _,
                )
            } {
                Ok(img) => {
                    let dets = tokio::task::block_in_place(|| det.blocking_lock()).detect(&img);

                    for det in &dets {
                        // Extract camera calibration values from the [CalibratedModel]
                        let OpenCVModel5 { fx, fy, cx, cy, .. } =
                            if let GenericModel::OpenCVModel5(model) = model.inner_model() {
                                model
                            } else {
                                panic!("camera model type not supported yet");
                            };

                        // Estimate tag pose with the camera calibration values
                        if let Some(pose) = det.estimate_tag_pose(&TagParams {
                            fx,
                            fy,
                            cx,
                            cy,
                            tagsize: TAG_SIZE,
                        }) {
                            // Extract the camera's translation and rotation matrices from the [Pose]
                            let cam_translation = pose.translation().data();
                            let cam_rotation = pose.rotation().data();

                            // Convert the camera's translation and rotation matrices into proper Rust datatypes
                            let translation: na::Translation3<f64> =
                                MatrixView3x1::from_slice(cam_translation)
                                    .into_owned()
                                    .into();
                            let cam_rotation = na::UnitQuaternion::from_matrix(
                                &MatrixView3::from_slice(cam_rotation).transpose(),
                            );

                            debug!(
                                "detected tag id {}: tl={cam_translation:?} ro={cam_rotation:?}",
                                det.id()
                            );

                            let tag_est_pos = na::Isometry3::from_parts(translation, cam_rotation);
                            //Try to get the tag's pose from the field layout
                            //let translation = *tag_translation * translation;
                            //let rotation = *tag_rotation * rotation;
                            //futures_executor::block_on(
                            //    manager
                            //        .pose_est
                            //        .add_transform_from_tag(tag_est_pos, det.id()),
                            //)
                            //.unwrap();

                            //return Some((translation, rotation, det.decision_margin() as f64));
                        }
                    }

                    if !dets.is_empty() {
                        tag_detected.set(true).await.unwrap();

                        delay
                            .set(proc_st_time.elapsed().as_millis_f64())
                            .await
                            .unwrap();
                    } else {
                        tag_detected.set(false).await.unwrap();

                        debug!("no tag detected");
                    }
                }
                Err(err) => {
                    tag_detected.set(false).await.unwrap();
                    error!("failed to convert image to C apriltags type: {err:?}");
                }
            }
        })
        .await;

        panic!("It shouldn't be possible to the frame processing loop early");

        Ok(())
    }
}

pub struct CapriltagsPreproc {
    videoconvertscale: Arc<Element>,
    filter: Arc<Element>,
}
impl Preprocessor for CapriltagsPreproc {
    type Frame = Vec<u8>;
    type Subsys = CApriltagsDetector;

    fn new(pipeline: &gstreamer::Pipeline) -> Self {
        // The AprilTag preprocessing part:
        //  tee ! gamma ! videoconvertscale ! capsfilter ! appsink

        // Create the elements
        let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
        let filter = ElementFactory::make("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
                    .field("width", &1280)
                    .field("height", &720)
                    .field("format", "GRAY8")
                    .build(),
            )
            .build()
            .unwrap();

        // Add them to the pipeline
        pipeline.add_many([&videoconvertscale, &filter]).unwrap();

        Self {
            videoconvertscale: videoconvertscale.into(),
            filter: filter.into(),
        }
    }

    fn link(&self, src: Element, sink: Element) {
        let _ = Element::link_many([&src, &self.videoconvertscale, &self.filter, &sink]);
    }
    fn unlink(&self, src: Element, sink: Element) {
        let _ = Element::unlink_many([&src, &self.videoconvertscale, &self.filter, &sink]);
    }

    fn sampler(
        appsink: &gstreamer_app::AppSink,
        tx: watch::Sender<Option<Arc<Self::Frame>>>,
    ) -> Result<Option<()>, Error> {
        let sample = appsink
            .pull_sample()
            .map_err(|_| Error::FailedToPullSample)?;
        let buf = sample.buffer().unwrap();
        let buf = buf
            .to_owned()
            .into_mapped_buffer_readable()
            .unwrap()
            .to_vec();
        tx.send(Some(Arc::new(buf))).unwrap();

        Ok(Some(()))
    }
}
