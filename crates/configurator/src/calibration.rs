use chalkydri::subsystems::calibration::CALIB;
use std::time::Duration;

use aprilgrid::{detector::TagDetector, TagFamily};
use camera_intrinsic_calibration::{
    board::{create_default_6x6_board, Board},
    detected_points::FrameFeature,
    types::CalibParams,
    util::*,
};
use camera_intrinsic_model::{GenericModel, OpenCVModel5};

use tokio::time::Instant;

pub struct CalibratedModel {
    model: GenericModel<f64>,
}
impl CalibratedModel {
    pub fn from_str(calib: String) -> Self {
        let model = serde_json::from_str(&calib).unwrap();
        Self { model }
    }

    pub const fn inner_model(&self) -> GenericModel<f64> {
        self.model
    }
}

const MIN_CORNERS: usize = 24;

/// A camera calibrator
pub struct Calibrator {
    det: TagDetector,
    board: Board,
    frame_feats: Vec<FrameFeature>,
    cam_model: GenericModel<f64>,
    start: Instant,
    stream: rerun::RecordingStream,
}
impl Calibrator {
    /// Initialize the [Calibrator]
    pub fn new() -> Self {
        let (stream, _mem_sink) = rerun::RecordingStreamBuilder::new("calibration")
            .memory()
            .unwrap();
        stream
            .log_static("/", &rerun::ViewCoordinates::RDF())
            .unwrap();
        Self {
            det: TagDetector::new(&TagFamily::T36H11, None),
            board: create_default_6x6_board(),
            frame_feats: Vec::new(),
            cam_model: GenericModel::OpenCVModel5(OpenCVModel5::zeros()),
            start: Instant::now(),
            stream,
        }
    }

    /// Attempt to capture a frame and process it
    ///
    /// Returns `true` until enough frames have been processed to run calibration.
    pub fn process(&mut self) -> usize {
        if let Some(img) = CALIB.read().clone().map_or(None, |c| c.recv_timeout(Duration::from_millis(10)).ok()) {
            if let Some(frame_feat) =
                camera_intrinsic_calibration::data_loader::image_to_option_feature_frame(
                    &self.det,
                    &img.0,
                    &create_default_6x6_board(),
                    MIN_CORNERS,
                    img.1.as_nanos() as i64,
                )
            {
                self.frame_feats.push(frame_feat);
            }
        }
        std::thread::sleep(Duration::from_millis(10));

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
                &self.stream,
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
