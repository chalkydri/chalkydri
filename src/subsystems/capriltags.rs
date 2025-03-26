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
use gstreamer::ElementFactory;
use gstreamer::prelude::GstBinExtManual;
use gstreamer::{Buffer, Caps, Element};
use minint::NtConn;
use nalgebra as na;
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

use crate::calibration::CalibratedModel;
use crate::error::Error;
use crate::pose::PoseEstimator;
use crate::{Cfg, Nt};
use crate::{Subsystem, config, subsystems::frame_proc_loop};

use super::SubsysManager;

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
    type Error = Box<dyn std::error::Error + Send>;

    fn preproc(
        _cam_config: config::Camera,
        pipeline: &gstreamer::Pipeline,
    ) -> Result<(gstreamer::Element, gstreamer::Element), Self::Error> {
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

        // Link them
        Element::link_many([&videoconvertscale, &filter]).unwrap();

        Ok((videoconvertscale, filter))
    }
    async fn init() -> Result<Self, Self::Error> {
        let det = Detector::builder()
            .add_family_bits(Family::tag_36h11(), 3)
            .build()
            .unwrap();

        Ok(Self {
            det: Arc::new(Mutex::new(det)),
        })
    }
    async fn process(
        &self,
        manager: SubsysManager,
        nt: NtConn,
        cam_config: config::Camera,
        rx: watch::Receiver<Option<Vec<u8>>>,
    ) -> Result<Self::Output, Self::Error> {
        let model = CalibratedModel::new(cam_config.calib.unwrap());
        let cam_name = cam_config.name.clone();

        // Publish NT topics we'll use
        let mut delay = Nt
            .publish::<f64>(&format!("/chalkydri/cameras/{cam_name}/delay"))
            .await
            .unwrap();
        let mut tag_detected = Nt
            .publish::<bool>(&format!("/chalkydri/cameras/{cam_name}/tag_detected"))
            .await
            .unwrap();

        debug!("running frame processing loop...");
        let det = self.det.clone();

        frame_proc_loop(rx, async move |buf| {
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
                            let cam_translation = pose.translation().data().to_vec();
                            let cam_rotation = pose.rotation().data().to_vec();

                            // Convert the camera's translation and rotation matrices into proper Rust datatypes
                            let translation = VecF64::<3>::new(
                                cam_translation[0],
                                cam_translation[1],
                                cam_translation[2],
                            );
                            let cam_rotation = Rotation3F64::try_from_mat(MatF64::<3, 3>::new(
                                cam_rotation[0],
                                cam_rotation[1],
                                cam_rotation[2],
                                cam_rotation[3],
                                cam_rotation[4],
                                cam_rotation[5],
                                cam_rotation[6],
                                cam_rotation[7],
                                cam_rotation[8],
                            ))
                            .unwrap();

                            debug!(
                                "detected tag id {}: tl={cam_translation:?} ro={cam_rotation:?}",
                                det.id()
                            );

                            let tag_est_pos = Isometry3F64::from_translation_and_rotation(
                                translation,
                                cam_rotation,
                            );
                            //Try to get the tag's pose from the field layout
                            //let translation = *tag_translation * translation;
                            //let rotation = *tag_rotation * rotation;
                            futures_executor::block_on(
                                manager
                                    .pose_est
                                    .add_transform_from_tag(tag_est_pos, det.id()),
                            )
                            .unwrap();

                            //return Some((translation, rotation, det.decision_margin() as f64));
                        }
                    }

                    if !dets.is_empty() {
                        futures_executor::block_on(async {
                            tag_detected.set(true).await;

                            delay.set(proc_st_time.elapsed().as_millis_f64()).await;
                        });
                    } else {
                        futures_executor::block_on(async {
                            tag_detected.set(false).await;
                        });

                        debug!("no tag detected");
                    }
                }
                Err(err) => {
                    futures_executor::block_on(async {
                        tag_detected.set(false).await;
                    });
                    error!("failed to convert image to C apriltags type: {err:?}");
                }
            }
        })
        .await;

        panic!("It shouldn't be possible to the frame processing loop early");

        Ok(())
    }
}
