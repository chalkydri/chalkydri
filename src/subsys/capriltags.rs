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
use camera_intrinsic_model::{GenericModel, OpenCVModel5};
use rapier3d::math::{Matrix, Rotation, Translation};
use rapier3d::na::{Matrix3, Vector3};
use rapier3d::na::Quaternion;
use rerun::components::{PinholeProjection, PoseRotationQuat, ViewCoordinates};
use rerun::external::re_types;
use rerun::{Boxes2D, Mat3x3, Points2D, Position2D};

use crate::calibration::CalibratedModel;
use crate::Subsystem;

const TAG_SIZE: f64 = 165.1;

pub struct CApriltagsDetector {
    det: apriltag::Detector,
    layout: AprilTagFieldLayout,
    model: CalibratedModel,
}
impl<'fr> Subsystem<'fr> for CApriltagsDetector {
    type Config = ();
    type Output = (Vec<f64>, Vec<f64>);
    type Error = Box<dyn std::error::Error + Send>;

    async fn init(cfg: Self::Config) -> Result<Self, Self::Error> {
        let model = CalibratedModel::new();
        let layout: AprilTagFieldLayout =
            serde_json::from_reader(File::open("layout.json").unwrap()).unwrap();
        let det = Detector::builder()
            .add_family_bits(Family::tag_36h11(), 3)
            .build()
            .unwrap();

        Ok(Self { det, layout, model })
    }
    fn process(
        &mut self,
        buf: crate::subsystem::Buffer,
        rr: rerun::RecordingStream,
    ) -> Result<Self::Output, Self::Error> {
        let img_rgb = DynamicImage::ImageRgb8(RgbImage::from_vec(1280, 720, buf.to_vec()).unwrap());
        let img_gray = img_rgb.grayscale();
        let buf = img_gray.as_luma8().unwrap();
        let img = Image::from_image_buffer(buf);
        let dets = self.det.detect(&img);

        let poses: Vec<_> = dets
            .iter()
            .filter_map(|det| {
                let OpenCVModel5 { fx, fy, cx, cy, .. } = if let GenericModel::OpenCVModel5(model) = self.model.inner_model() {
                    model
                } else {
                    panic!("camera model type not supported yet");
                };

                let pose = det.estimate_tag_pose(&TagParams {
                    fx,
                    fy,
                    cx,
                    cy,
                    tagsize: TAG_SIZE,
                }).unwrap();

                let cam_translation = pose.translation().data().to_vec();
                let cam_translation =
                    Translation::new(cam_translation[0], cam_translation[1], cam_translation[2]);

                let cam_rotation = pose.rotation().data().to_vec();
                let cam_rotation = Rotation::from_matrix(&Matrix::from_vec(cam_rotation));

                //let (rotation, translation) = CalibratedModel::new().determine_pose(det.corners().iter().map(|c| (c[0], c[1])).collect::<Vec<_>>());
                //let rotation = Rotation::from_euler_angles(rotation.0, rotation.1, rotation.2);
                //let quat = rerun::Quaternion::from_wxyz([rotation.w as f32, rotation.i as f32, rotation.j as f32, rotation.k as f32]);
                //let quat2 = rerun::Quaternion::from_wxyz([cam_rotation.w as f32, cam_rotation.i as f32, cam_rotation.j as f32, cam_rotation.k as f32]);
                //rr.log("/image/tag_rust", &rerun::Transform3D::from_translation_rotation(rerun::Vec3D::new(translation.0 as f32, translation.1 as f32, translation.2 as f32), quat)).unwrap();
                //rr.log("/image/tag_c", &rerun::Transform3D::from_translation_rotation(rerun::Vec3D::new(cam_translation.x as f32, cam_translation.y as f32, cam_translation.z as f32), quat2)).unwrap();

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
                    rr.log(
                        "/tag",
                        &Points2D::new(
                            det.corners()
                                .iter()
                                .map(|c| Position2D::new(c[0] as f32, c[1] as f32)),
                        ),
                    )
                    .unwrap();
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
