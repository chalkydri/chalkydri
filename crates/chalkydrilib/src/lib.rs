extern crate sophus;

use sophus::timeseries::TimeSeries;
use sophus::autodiff::linalg::VecF64;
use sophus::lie::{Rotation3F64, Isometry3F64};
use core::f64::consts::FRAC_PI_4;

pub struct PoseEstimator {
}
impl PoseEstimator {
    pub fn new() {
        optimize_nlls();
    }
}
