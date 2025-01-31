use aprilgrid::detector::{DetectorParams, TagDetector};
use aprilgrid::TagFamily;
use camera_intrinsic_calibration::detected_points::FrameFeature;
use camera_intrinsic_calibration::{self as calib, optimization::homography};
use camera_intrinsic_model::{self as model, CameraModel, GenericModel};

use model::opencv5::OpenCVModel5;
use model::{model_from_json, model_to_json};
use rapier3d::na::Vector2;
use sqpnp_simple::sqpnp_solve;

pub struct CalibratedModel {
    model: GenericModel<f64>,
}
impl CalibratedModel {
    pub fn new() -> Self {
        // Load the camera model
        let model = model_from_json("cam0.json");

        Self { model }
    }

    pub const fn inner_model(&self) -> GenericModel<f64> {
        self.model
    }

    /// Returns rotation, then translation
    /// <https://stackoverflow.com/a/75871586>
    pub fn determine_pose(&self, points: Vec<(f64, f64)>) -> ((f64, f64, f64), (f64, f64, f64)) {
        // Unproject the 2D coordinates into 3D coordinates
        let undistorted = self.model.unproject(
            &points
                .iter()
                .map(|p| Vector2::new(p.0 as f64, p.1 as f64))
                .collect::<Vec<_>>(),
        );

        info!("{:?}", undistorted);

        sqpnp_solve(
            &undistorted
                .iter()
                .map(|p| {
                    let p = p.unwrap();
                    (p[0], p[1], p[2])
                })
                .collect::<Vec<_>>(),
            &points.iter().map(|p| (p.0 as f64, p.1)).collect::<Vec<_>>(),
        )
        .unwrap()

        // Solve PnP
        //sqpnp_solve();
    }
}
