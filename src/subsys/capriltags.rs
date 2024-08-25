//!
//! Subsystem for the official AprilTags C library
//!

// TODO: implement this
// There's actually already a decent Rust binding we can use.
// There's an example here: <https://github.com/jerry73204/apriltag-rust/blob/master/apriltag/examples/detector.rs>
//
// <https://www.chiefdelphi.com/t/frc-blog-technology-updates-past-present-future-and-beyond-apriltags-and-new-radio/440931>
// According to this post on CD, we're doing the 36h11 tag family now.

use apriltag::Detector;

use crate::Subsystem;

pub struct CApriltagsDetector {
    det: apriltag::Detector,
}
impl Subsystem<'_, ()> for CApriltagsDetector {
    type Processor = Self;
    type Config = ();

    async fn init() -> Result<Self, Box<dyn std::error::Error>> {
    }
}
