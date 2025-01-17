//!
//! Subsystem for the official AprilTags C library
//!

// TODO: implement this
// There's actually already a decent Rust binding we can use.
// There's an example here: <https://github.com/jerry73204/apriltag-rust/blob/master/apriltag/examples/detector.rs>
//
// <https://www.chiefdelphi.com/t/frc-blog-technology-updates-past-present-future-and-beyond-apriltags-and-new-radio/440931>
// According to this post on CD, we're doing the 36h11 tag family now.

use actix::{Actor, Addr, Arbiter, Handler, SyncArbiter, SyncContext};
use apriltag::{Detector, Family, Image, TagParams};
use apriltag_image::image::{DynamicImage, RgbImage};
use apriltag_image::prelude::*;

use crate::{ProcessFrame, Subsystem};

const TAG_PARAMS: TagParams = TagParams {
    tagsize: 1.0,
    fx: 1.0,
    fy: 1.0,
    cx: 1.0,
    cy: 1.0,
};

pub struct CApriltagsDetector {
    det: apriltag::Detector,
}
impl Subsystem<'_> for CApriltagsDetector {
    type Config = ();
    type Output = Vec<(Vec<f64>, Vec<f64>)>;
    type Error = Box<dyn std::error::Error + Send>;

    async fn init(_cfg: Self::Config) -> Result<Addr<Self>, Self::Error> {
        Ok(SyncArbiter::start(1, || {
            let det = Detector::builder()
                .add_family_bits(Family::tag_36h11(), 3)
                .build()
                .unwrap();

            Self { det }
        }))
    }

    fn handle(
        &mut self,
        msg: crate::subsystem::ProcessFrame<Self::Output, Self::Error>,
        ctx: &mut <Self as Actor>::Context,
    ) -> Result<Self::Output, Self::Error> {
        let img_rgb =
            DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, msg.buf.to_vec()).unwrap());
        let img_gray = img_rgb.grayscale();
        let buf = img_gray.as_luma8().unwrap();
        let img = Image::from_image_buffer(buf);
        let dets = self.det.detect(&img);

        Ok(dets
            .iter()
            .map(|det| {
                let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();
                let translation = pose.translation().data().to_vec();
                let rotation = pose.rotation().data().to_vec();
                (translation, rotation)
            })
            .collect())
    }
}

// impl CApriltagsDetector {
//     pub fn new() -> Self {
//         let det = Detector::builder()
//             .add_family_bits(Family::tag_36h11(), 3)
//             .build()
//             .unwrap();

//         Self { det }
//     }
//     pub fn detect(&mut self, buf: Vec<u8>) {
//         let img_rgb =
//             DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, buf.to_vec()).unwrap());
//         let img_gray = img_rgb.grayscale();
//         let buf = img_gray.as_luma8().unwrap();
//         let img = Image::from_image_buffer(buf);
//         //img_rgb.save("skibidi.png").unwrap();
//         let dets = self.det.detect(&img);
//         for det in dets {
//             let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();
//             dbg!(pose.rotation(), pose.translation());
//         }
//     }
// }

// impl Actor for CApriltagsDetector {
//     type Context = SyncContext<Self>;
// }

// impl Handler<ProcessFrame<Vec<(Vec<f64>, Vec<f64>)>, Box<dyn std::error::Error + Send>>>
//     for CApriltagsDetector
// {
//     type Result = Result<Vec<(Vec<f64>, Vec<f64>)>, Box<dyn std::error::Error + Send>>;

//     fn handle(
//         &mut self,
//         msg: ProcessFrame<<Self as Subsystem>::Output, Box<dyn std::error::Error + Send>>,
//         _ctx: &mut Self::Context,
//     ) -> Self::Result {
//         let img_rgb =
//             DynamicImage::ImageRgb8(RgbImage::from_vec(1920, 1080, msg.buf.to_vec()).unwrap());
//         let img_gray = img_rgb.grayscale();
//         let buf = img_gray.as_luma8().unwrap();
//         let img = Image::from_image_buffer(buf);
//         let dets = self.det.detect(&img);

//         Ok(dets
//             .iter()
//             .map(|det| {
//                 let pose = det.estimate_tag_pose(&TAG_PARAMS).unwrap();
//                 let translation = pose.translation().data().to_vec();
//                 let rotation = pose.rotation().data().to_vec();
//                 (translation, rotation)
//             })
//             .collect())
//     }
// }
