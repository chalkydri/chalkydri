use chalkydri::subsystems::calibration::CALIB;
use serde::{Deserialize, Serialize};

use aprilgrid::{TagFamily, detector::TagDetector};
use camera_intrinsic_calibration::{
    board::{Board, create_default_6x6_board},
    detected_points::FrameFeature,
    types::CalibParams,
    util::*,
};
use camera_intrinsic_model::{GenericModel, OpenCVModel5};

use crate::monitor::MONITOR;

#[derive(Default, Deserialize, Serialize)]
pub struct CalibratedModel {
    model: Option<GenericModel<f64>>,
}
impl CalibratedModel {
    pub fn from_str(calib: String) -> Self {
        let model = serde_json::from_str(&calib).unwrap();
        Self { model }
    }

    pub fn inner_model(&self) -> GenericModel<f64> {
        self.model.unwrap()
    }
}

const MIN_CORNERS: usize = 24;

/// A camera calibrator
pub struct Calibrator {
    det: TagDetector,
    board: Board,
    frame_feats: Vec<FrameFeature>,
    cam_model: GenericModel<f64>,
}
impl Calibrator {
    /// Initialize the [Calibrator]
    pub fn new() -> Self {
        MONITOR
            .stream
            .log_static("/", &rerun::ViewCoordinates::RDF())
            .unwrap();
        Self {
            det: TagDetector::new(&TagFamily::T36H11, None),
            board: create_default_6x6_board(),
            frame_feats: Vec::new(),
            cam_model: GenericModel::OpenCVModel5(OpenCVModel5::zeros()),
        }
    }

    /// Attempt to capture a frame and process it
    ///
    /// Returns `true` until enough frames have been processed to run calibration.
    pub fn process(&mut self) -> usize {
        let img = CALIB.lock().take();

        if let Some(img) = img {
            if let Some(frame_feat) =
                camera_intrinsic_calibration::data_loader::image_to_option_feature_frame(
                    &self.det,
                    &img.0,
                    &self.board,
                    MIN_CORNERS,
                    img.1.as_nanos() as i64,
                )
            {
                self.frame_feats.push(frame_feat);
                let points2d = self
                    .frame_feats
                    .iter()
                    .map(|frame_feat| {
                        frame_feat
                            .features
                            .values()
                            .map(|feat| (feat.p2d.x, feat.p2d.y))
                    })
                    .flatten();
                let points3d = self
                    .frame_feats
                    .iter()
                    .map(|frame_feat| {
                        frame_feat
                            .features
                            .values()
                            .map(|feat| (feat.p3d.x, feat.p3d.y, feat.p3d.z))
                    })
                    .flatten();
                MONITOR
                    .stream
                    .log_with_static("/cam/frame/points2d", true, &rerun::Points2D::new(points2d))
                    .unwrap();
                MONITOR
                    .stream
                    .log_with_static("/cam/frame/points3d", true, &rerun::Points3D::new(points3d))
                    .unwrap();
            }
        }

        self.frame_feats.len()
    }

    /// Calibrate
    pub fn calibrate(&mut self) -> Option<GenericModel<f64>> {
        let mut calib_res = None;

        for i in 0..5 {
            calib_res = init_and_calibrate_one_camera(
                0,
                &[self
                    .frame_feats
                    .clone()
                    .into_iter()
                    .map(|f| Some(f))
                    .collect()],
                &self.cam_model,
                &CalibParams {
                    one_focal: false,
                    fixed_focal: None,
                    disabled_distortion_num: 0,
                },
                i > 1,
            );
            if calib_res.is_some() {
                break;
            }
        }

        self.frame_feats.clear();

        if calib_res.is_none() {
            None
        } else {
            Some(calib_res.unwrap().0)
        }
    }
}
