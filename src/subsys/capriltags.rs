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

use actix::{Actor, Addr, Arbiter, Handler, SyncArbiter, SyncContext};
use apriltag::{Detector, Family, Image, TagParams};
use apriltag_image::image::{DynamicImage, RgbImage};
use apriltag_image::prelude::*;
use nalgebra::{Isometry3, Matrix3, Quaternion, Rotation3, Translation3, UnitQuaternion};

use crate::{ProcessFrame, Subsystem};

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
    type Output = Vec<(Vec<f64>, Vec<f64>)>;
    type Error = Box<dyn std::error::Error + Send>;

    async fn init(_cfg: Self::Config) -> Result<Addr<Self>, Self::Error> {
        Ok(SyncArbiter::start(1, || {
            let layout: AprilTagFieldLayout = serde_json::from_reader(File::open("layout.json").unwrap()).unwrap();
            
            let det = Detector::builder()
                .add_family_bits(Family::tag_36h11(), 3)
                .build()
                .unwrap();

            Self { det, layout }
        }))
    }

    //fn handle(
    //    &mut self,
    //    msg: crate::subsystem::ProcessFrame<Self::Output, Self::Error>,
    //    ctx: &mut <Self as Actor>::Context,
    //) -> Result<Self::Output, Self::Error> {
    //    let img_rgb =
    //        DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, msg.buf.to_vec()).unwrap());
    //    let img_gray = img_rgb.grayscale();
    //    let buf = img_gray.as_luma8().unwrap();
    //    let img = Image::from_image_buffer(buf);
    //    let dets = self.det.detect(&img);

    //    Ok(dets
    //        .iter()
    //        .map(|det| {
    //            let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();
    //            let translation = pose.translation().data().to_vec();
    //            let rotation = pose.rotation().data().to_vec();
    //            (translation, rotation)
    //        })
    //        .collect())
    //}
}

 impl CApriltagsDetector {
     pub fn new() -> Self {
        let layout: AprilTagFieldLayout = serde_json::from_reader(File::open("layout.json").unwrap()).unwrap();
         let det = Detector::builder()
            .add_family_bits(Family::tag_36h11(), 3)
            .build()
            .unwrap();

        Self { det, layout }
    }
    pub fn detect(&mut self, buf: Vec<u8>) {
        let img_rgb =
            DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, buf.to_vec()).unwrap());
        let img_gray = img_rgb.grayscale();
        let buf = img_gray.as_luma8().unwrap();
        let img = Image::from_image_buffer(buf);
        //img_rgb.save("skibidi.png").unwrap();
        let dets = self.det.detect(&img);
        for det in dets {
            let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();
            dbg!(pose.rotation(), pose.translation());
        }
    }
}

impl Actor for CApriltagsDetector {
    type Context = SyncContext<Self>;
}

impl Handler<ProcessFrame<Vec<(Vec<f64>, Vec<f64>)>, Box<dyn std::error::Error + Send>>>
    for CApriltagsDetector
{
    type Result = Result<Vec<(Vec<f64>, Vec<f64>)>, Box<dyn std::error::Error + Send>>;

    fn handle(
        &mut self,
        msg: ProcessFrame<<Self as Subsystem>::Output, Box<dyn std::error::Error + Send>>,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let img_rgb =
            DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, msg.buf.to_vec()).unwrap());
        let img_gray = img_rgb.grayscale();
        let buf = img_gray.as_luma8().unwrap();
        let img = Image::from_image_buffer(buf);
        let dets = self.det.detect(&img);

        Ok(dets
            .iter()
            .filter_map(|det| {
                let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();

                let cam_translation = pose.translation().data().to_vec();
                let cam_translation = Translation3::new(cam_translation[0], cam_translation[1], cam_translation[2]);

                let cam_rotation = pose.rotation().data().to_vec();
                let cam_rotation = Rotation3::from_matrix(&Matrix3::from_vec(cam_rotation));

                let tag_translation: Translation3<f64>;
                let tag_rotation: Quaternion<f64>;

                for Tag { id, pose: Pose { translation, rotation: Rotation { quaternion } } } in self.layout.tags.clone() {
                    if det.id() == (id as usize) {
                        tag_translation = Translation3::new(translation.x, translation.y, translation.z);
                        tag_rotation = Quaternion::new(quaternion.w, quaternion.x, quaternion.y, quaternion.z);

                        let translation = tag_translation * cam_translation;
                        let rotation = UnitQuaternion::from_quaternion(tag_rotation * *UnitQuaternion::from_rotation_matrix(&cam_rotation)).to_rotation_matrix();

                        return Some((translation.to_homogeneous().data.as_slice().to_vec(), rotation.to_homogeneous().data.as_slice().to_vec()));
                    }
                }

                None
            })
            .collect())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AprilTagFieldLayout {
    pub tags: Vec<Tag>,
    pub field: Field,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    #[serde(rename = "ID")]
    pub id: i64,
    pub pose: Pose,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    pub translation: Translation,
    pub rotation: Rotation,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rotation {
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
