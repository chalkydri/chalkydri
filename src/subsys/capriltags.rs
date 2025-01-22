//!
//! Subsystem for the official AprilTags C library
//!

// TODO: implement this
// There's actually already a decent Rust binding we can use.
// There's an example here: <https://github.com/jerry73204/apriltag-rust/blob/master/apriltag/examples/detector.rs>
//
// <https://www.chiefdelphi.com/t/frc-blog-technology-updates-past-present-future-and-beyond-apriltags-and-new-radio/440931>
// According to this post on CD, we're doing the 36h11 tag family now.

use std::fs::File;

use apriltag::{Detector, Family, Image, TagParams};
use apriltag_image::image::{DynamicImage, RgbImage};
use apriltag_image::prelude::*;
use cam_geom::{Pixels, Ray};
use rapier3d::math::{Matrix, Rotation, Translation};
use rapier3d::na::Matrix3;
use rapier3d::na::Quaternion;

use crate::Subsystem;

const TAG_PARAMS: TagParams = TagParams {
    tagsize: 1.0,
    fx: 100.0,
    fy: 100.0,
    cx: 1920.0,
    cy: 1080.0,
};

pub struct CApriltagsDetector {
    det: apriltag::Detector,
    layout: AprilTagFieldLayout,
}
impl<'fr> Subsystem<'fr> for CApriltagsDetector {
    type Config = ();
    type Output = (Vec<f64>, Vec<f64>);
    type Error = Box<dyn std::error::Error + Send>;


    async fn init(cfg: Self::Config) -> Result<Self, Self::Error> {
            let layout: AprilTagFieldLayout =
                serde_json::from_reader(File::open("layout.json").unwrap()).unwrap();
            let det = Detector::builder()
                .add_family_bits(Family::tag_36h11(), 3)
                .build()
                .unwrap();

            Ok(Self { det, layout })
    }
    fn process(&mut self, buf: crate::subsystem::Buffer) -> Result<Self::Output, Self::Error> {
        let img_rgb =
            DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, buf.to_vec()).unwrap());
        let img_gray = img_rgb.grayscale();
        let buf = img_gray.as_luma8().unwrap();
        let img = Image::from_image_buffer(buf);
        let dets = self.det.detect(&img);

        let poses: Vec<_> = dets
            .iter()
            .filter_map(|det| {
                let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();

                let cam_translation = pose.translation().data().to_vec();
                let cam_translation =
                    Translation::new(cam_translation[0], cam_translation[1], cam_translation[2]);

                let cam_rotation = pose.rotation().data().to_vec();
                let cam_rotation = Rotation::from_matrix(&Matrix::from_vec(cam_rotation));

                let tag_translation: Translation<f64>;
                let tag_rotation: Rotation<f64>;

                for LayoutTag {
                    id,
                    pose:
                        LayoutPose {
                            translation,
                            rotation: LayoutRotation { quaternion },
                        },
                } in self.layout.tags.clone()
                {
                    if det.id() == (id as usize) {
                        tag_translation =
                            Translation::new(translation.x, translation.y, translation.z);
                        tag_rotation = Rotation::from_quaternion(Quaternion::new(
                            quaternion.w,
                            quaternion.x,
                            quaternion.y,
                            quaternion.z,
                        ));

                        let translation = tag_translation * cam_translation;
                        let rotation = tag_rotation * cam_rotation;
                        return Some((translation, rotation, det.decision_margin() as f64));
                    }
                }

                None
            })
            .collect();

        let mut weighted_avg_translation = Translation::new(0.0f64, 0.0, 0.0);
        let mut weighted_avg_rotation = Quaternion::new(0.0f64, 0.0, 0.0, 0.0);

        for pose in poses.iter() {
            weighted_avg_translation.x += pose.0.x * pose.2;
            weighted_avg_translation.y += pose.0.y * pose.2;
            weighted_avg_translation.z += pose.0.z * pose.2;

            weighted_avg_rotation.w += pose.1.w * pose.2;
            weighted_avg_rotation.i += pose.1.i * pose.2;
            weighted_avg_rotation.j += pose.1.j * pose.2;
            weighted_avg_rotation.k += pose.1.k * pose.2;
        }

        weighted_avg_translation.x /= poses.len() as f64;
        weighted_avg_translation.x /= poses.len() as f64;
        weighted_avg_translation.x /= poses.len() as f64;

        weighted_avg_rotation.w /= poses.len() as f64;
        weighted_avg_rotation.i /= poses.len() as f64;
        weighted_avg_rotation.j /= poses.len() as f64;
        weighted_avg_rotation.k /= poses.len() as f64;

        Ok((
            weighted_avg_translation.vector.data.as_slice().to_vec(),
            weighted_avg_rotation.vector().data.into_slice().to_vec(),
        ))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AprilTagFieldLayout {
    pub tags: Vec<LayoutTag>,
    pub field: Field,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTag {
    #[serde(rename = "ID")]
    pub id: i64,
    pub pose: LayoutPose,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPose {
    pub translation: LayoutTranslation,
    pub rotation: LayoutRotation,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTranslation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRotation {
    pub quaternion: LayoutQuaternion,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub length: f64,
    pub width: f64,
}
