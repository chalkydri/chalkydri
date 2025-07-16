extern crate sophus;

use core::f64::consts::FRAC_PI_4;
use sophus::autodiff::linalg::VecF64;
use sophus::lie::{Isometry3F64, Rotation3F64};
use sophus::timeseries::TimeSeries;

pub struct PoseEstimator {}
impl PoseEstimator {
    pub fn new() {
        optimize_nlls();
    }
}
