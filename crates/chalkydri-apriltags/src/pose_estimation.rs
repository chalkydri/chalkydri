use cam_geom::*;
use nalgebra::{Perspective3, Point3};

pub fn pose_estimation(intrinsics: IntrinsicParametersPerspective<f32>) {
    let pose = ExtrinsicParameters::from_view(&camcenter, &lookat, &up);
    let intrinsics = IntrinsicParametersPerspective::from(PerspectiveParams {
        fx: 100.0,
        fy: 100.0,
        skew: 0.0,
        cx: 640.0,
        cy: 480.0,
    });
    let camera = Camera::new(intrinsics, pose);
}
