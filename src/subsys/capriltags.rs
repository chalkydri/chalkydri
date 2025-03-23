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
use sophus_lie::{Isometry3F64, Rotation2F64, Rotation3F64};
use std::time::Instant;
use tokio::sync::watch;

use crate::Cfg;
use crate::calibration::CalibratedModel;
use crate::error::Error;
use crate::pose::PoseEstimator;
use crate::{Subsystem, config, subsystem::frame_proc_loop};

const TAG_SIZE: f64 = 0.1651;

pub struct CApriltagsDetector {
    det: apriltag::Detector,
    model: CalibratedModel,
    name: String,
    pose_est: PoseEstimator,
    layout: HashMap<usize, Isometry3F64>,
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
    async fn init(cam_config: config::Camera) -> Result<Self, Self::Error> {
        let model = CalibratedModel::new(cam_config.calib.unwrap());

        let subsys_cfg = cam_config.subsystems.capriltags;
        let default_layout = AprilTagFieldLayout {
            tags: Vec::new(),
            field: Field {
                width: 0.0,
                length: 0.0,
            },
        };
        let layouts = Cfg.read().await.field_layouts.clone().unwrap();
        let layout = layouts
            .get(&subsys_cfg.field_layout.unwrap_or_default())
            .unwrap_or(&default_layout);
        let det = Detector::builder()
            .add_family_bits(Family::tag_36h11(), 3)
            .build()
            .unwrap();

        let mut pose_est = PoseEstimator::new().await.unwrap();
        let layout = layout.load(&mut pose_est).await.unwrap();
        pose_est.nt_loop().await;

        Ok(Self {
            model,
            det,
            name: cam_config.name,
            pose_est,
            layout,
        })
    }
    async fn process(
        &mut self,
        nt: NtConn,
        rx: watch::Receiver<Option<Buffer>>,
    ) -> Result<Self::Output, Self::Error> {
        let cam_name = self.name.clone();

        // Publish NT topics we'll use
        let mut translation = nt
            .publish::<Vec<f64>>(&format!("/chalkydri/robot_pose/{cam_name}/translation"))
            .await
            .unwrap();
        let mut rotation = nt
            .publish::<Vec<f64>>(&format!("/chalkydri/robot_pose/{cam_name}/rotation"))
            .await
            .unwrap();
        let mut delay = nt
            .publish::<f64>(&format!("/chalkydri/robot_pose/{cam_name}/delay"))
            .await
            .unwrap();
        let mut tag_detected = nt
            .publish::<bool>(&format!("/chalkydri/robot_pose/{cam_name}/tag_detected"))
            .await
            .unwrap();

        debug!("running frame processing loop...");
        frame_proc_loop(rx, async |frame| {
            let proc_st_time = Instant::now();

            if let Ok(buf) = frame.into_mapped_buffer_readable() {
                debug!("loading image...");
                match unsafe { Image::from_luma8(1280, 720, buf.as_ptr() as *mut _) } {
                    Ok(img) => {
                        let dets = self.det.detect(&img);

                        for det in &dets {
                                // Extract camera calibration values from the [CalibratedModel]
                                let OpenCVModel5 { fx, fy, cx, cy, .. } =
                                    if let GenericModel::OpenCVModel5(model) = self.model.inner_model() {
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
                                    let cam_rotation =
                                        Rotation3F64::try_from_mat(MatF64::<3, 3>::new(
                                                cam_rotation[0], cam_rotation[1], cam_rotation[2],
                                                cam_rotation[3], cam_rotation[4], cam_rotation[5],
                                                cam_rotation[6], cam_rotation[7], cam_rotation[8],
                                        )).unwrap();

                                    debug!(
                                        "detected tag id {}: tl={cam_translation:?} ro={cam_rotation:?}",
                                        det.id()
                                    );


                                    let tag_est_pos = Isometry3F64::from_translation_and_rotation(translation, cam_rotation);
                                    //Try to get the tag's pose from the field layout
                                    if let Some(tag_field_pos) =
                                        self.layout.get(&det.id())
                                    {
                                        //let translation = *tag_translation * translation;
                                        //let rotation = *tag_rotation * rotation;
                                        self.pose_est.add_transform(tag_est_pos, *tag_field_pos);



                                        //return Some((translation, rotation, det.decision_margin() as f64));
                                    }
                                }
                        }


                        if !dets.is_empty() {
                            tag_detected.set(true).await;

                            delay.set(proc_st_time.elapsed().as_millis_f64()).await;
                        } else {
                            tag_detected.set(false).await;

                            debug!("no tag detected");
                        }
                    }
                    Err(err) => {
                        tag_detected.set(false).await;
                        error!("failed to convert image to C apriltags type: {err:?}");
                    }
                }
            } else {
            }
        })
        .await;

        panic!("It shouldn't be possible to the frame processing loop early");

        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AprilTagFieldLayout {
    pub tags: Vec<LayoutTag>,
    pub field: Field,
}
impl AprilTagFieldLayout {
    pub async fn load(
        &self,
        pose_est: &mut PoseEstimator,
    ) -> Result<HashMap<usize, Isometry3F64>, Error> {
        let mut tags: HashMap<usize, Isometry3F64> = HashMap::new();
        for LayoutTag {
            id,
            pose:
                LayoutPose {
                    translation,
                    rotation: LayoutRotation { quaternion },
                },
        } in self.tags.clone()
        {
            // Turn the field layout values into Rust datatypes
            let translation = VecF64::<3>::new(translation.x, translation.y, translation.z);
            let rotation = na::UnitQuaternion::from_quaternion(na::Quaternion::new(
                quaternion.x,
                quaternion.y,
                quaternion.z,
                quaternion.w,
            ))
            .to_rotation_matrix();
            let rotation = Rotation3F64::try_from_mat(rotation.matrix()).unwrap();

            let isometry = Isometry3F64::from_translation_and_rotation(translation, rotation);

            tags.insert(id as usize, isometry);
        }

        Ok(tags)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutTag {
    #[serde(rename = "ID")]
    pub id: i64,
    pub pose: LayoutPose,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutPose {
    pub translation: LayoutTranslation,
    pub rotation: LayoutRotation,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutTranslation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutRotation {
    pub quaternion: LayoutQuaternion,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutQuaternion {
    #[serde(rename = "W")]
    pub w: f64,
    #[serde(rename = "X")]
    pub x: f64,
    #[serde(rename = "Y")]
    pub y: f64,
    #[serde(rename = "Z")]
    pub z: f64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub length: f64,
    pub width: f64,
}
