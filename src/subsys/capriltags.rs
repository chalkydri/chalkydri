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
use std::fs::File;
use std::path::Path;

use apriltag::{Detector, Family, Image, TagParams};
use apriltag_image::image::{DynamicImage, GrayImage};
use apriltag_image::prelude::*;
use camera_intrinsic_model::{GenericModel, OpenCVModel5};
use gstreamer::prelude::GstBinExtManual;
use gstreamer::{Buffer, Caps, Element};
use gstreamer::{ElementFactory, FlowSuccess, State};
use minint::{NtConn, NtTopic};
use rapier3d::math::{Matrix, Rotation, Translation};
use rapier3d::na::Quaternion;
#[cfg(feature = "rerun")]
use re_sdk::external::re_types_core;
#[cfg(feature = "rerun")]
use re_types::{
    archetypes::{Boxes2D, Points2D},
    components::{PinholeProjection, PoseRotationQuat, Position2D, ViewCoordinates},
};
use std::time::Instant;
use tokio::sync::watch;

use crate::calibration::CalibratedModel;
use crate::{Subsystem, config, subsystem::frame_proc_loop};

const TAG_SIZE: f64 = 0.1651;

pub struct CApriltagsDetector {
    det: apriltag::Detector,
    layout: HashMap<u64, (Translation<f64>, Rotation<f64>)>,
    model: CalibratedModel,
    name: String,
}
impl Subsystem for CApriltagsDetector {
    const NAME: &'static str = "capriltags";

    type Config = config::CAprilTagsSubsys;
    type Output = ();
    type Error = Box<dyn std::error::Error + Send>;

    fn preproc(
        cam_config: config::Camera,
        pipeline: &gstreamer::Pipeline,
    ) -> Result<(gstreamer::Element, gstreamer::Element), Self::Error> {
        let config = cam_config.subsystems.capriltags;
        // The AprilTag preprocessing part:
        //  tee ! gamma ! videoconvertscale ! capsfilter ! appsink

        // Create the elements
        let gamma = ElementFactory::make("gamma")
            .property("gamma", &config.gamma.unwrap_or(1.0))
            .build()
            .unwrap();
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
        pipeline
            .add_many([&gamma, &videoconvertscale, &filter])
            .unwrap();

        // Link them
        Element::link_many([&gamma, &videoconvertscale, &filter]).unwrap();

        Ok((gamma, filter))
    }
    async fn init(cam_config: config::Camera) -> Result<Self, Self::Error> {
        let model = CalibratedModel::new();

        let subsys_cfg = cam_config.subsystems.capriltags;
        let default_layout = AprilTagFieldLayout {
            tags: Vec::new(),
            field: Field {
                width: 0.0,
                length: 0.0,
            },
        };
        let layout = subsys_cfg
            .field_layouts
            .get(&subsys_cfg.field_layout.unwrap_or_default())
            .unwrap_or(&default_layout);
        let layout = AprilTagFieldLayout::load(layout);
        let det = Detector::builder()
            .add_family_bits(Family::tag_36h11(), 3)
            .build()
            .unwrap();

        Ok(Self { model, layout, det, name: cam_config.name })
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

            debug!("loading image...");
            let img = GrayImage::from_vec(1280, 720, frame.map_readable().unwrap().to_vec()).unwrap();
                
            debug!("loading the image more...");
            let img = Image::from_image_buffer(&img.clone());
            
            let dets = self.det.detect(&img);

            let poses: Vec<_> = dets
                .iter()
                .filter_map(|det| {
                    // Extract camera calibration values from the [CalibratedModel]
                    let OpenCVModel5 { fx, fy, cx, cy, .. } =
                        if let GenericModel::OpenCVModel5(model) = self.model.inner_model() {
                            model
                        } else {
                            panic!("camera model type not supported yet");
                        };

                    // Estimate tag pose with the camera calibration values
                    let pose = det
                        .estimate_tag_pose(&TagParams {
                            fx,
                            fy,
                            cx,
                            cy,
                            tagsize: TAG_SIZE,
                        })
                        .unwrap();

                    // Extract the camera's translation and rotation matrices from the [Pose]
                    let cam_translation = pose.translation().data().to_vec();
                    let cam_rotation = pose.rotation().data().to_vec();

                    // Convert the camera's translation and rotation matrices into proper Rust datatypes
                    let cam_translation = Translation::new(
                        cam_translation[0],
                        cam_translation[1],
                        cam_translation[2],
                    );
                    let cam_rotation = Rotation::from_matrix(&Matrix::from_vec(cam_rotation));

                    // Try to get the tag's pose from the field layout
                    if let Some((tag_translation, tag_rotation)) =
                        self.layout.get(&(det.id() as u64))
                    {
                        let translation = tag_translation * cam_translation;
                        let rotation = tag_rotation * cam_rotation;
                        return Some((translation, rotation, det.decision_margin() as f64));
                    }

                    None
                })
                .collect();

            if poses.len() > 0 {
                let mut avg_translation = Translation::new(0.0f64, 0.0, 0.0);
                let mut avg_rotation = Quaternion::new(0.0f64, 0.0, 0.0, 0.0);

                for pose in poses.iter() {
                    avg_translation.x += pose.0.x;
                    avg_translation.y += pose.0.y;
                    avg_translation.z += pose.0.z;

                    avg_rotation.w += pose.1.w;
                    avg_rotation.i += pose.1.i;
                    avg_rotation.j += pose.1.j;
                    avg_rotation.k += pose.1.k;
                }

                avg_translation.x /= poses.len() as f64;
                avg_translation.x /= poses.len() as f64;
                avg_translation.x /= poses.len() as f64;

                avg_rotation.w /= poses.len() as f64;
                avg_rotation.i /= poses.len() as f64;
                avg_rotation.j /= poses.len() as f64;
                avg_rotation.k /= poses.len() as f64;

                let t = avg_translation.vector.data.as_slice().to_vec();
                let r = avg_rotation.vector().data.into_slice().to_vec();

                debug!("tag detected : {t:?} / {r:?}");

                translation.set(t).await;
                rotation.set(r).await;
                tag_detected.set(true).await;
                delay.set(proc_st_time.elapsed().as_millis_f64()).await;
            } else {
                debug!("no tag detected");
                tag_detected.set(false).await.unwrap();
            }
        })
        .await;
    debug!("loop done?");

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
    pub fn load(&self) -> HashMap<u64, (Translation<f64>, Rotation<f64>)> {
        let mut tags = HashMap::new();
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
            let tag_translation = Translation::new(translation.x, translation.y, translation.z);
            let tag_rotation = Rotation::from_quaternion(Quaternion::new(
                quaternion.w,
                quaternion.x,
                quaternion.y,
                quaternion.z,
            ));

            tags.insert(id as u64, (tag_translation, tag_rotation));
        }

        tags
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
