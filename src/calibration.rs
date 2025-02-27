use std::{collections::HashMap, path::Path, sync::Arc};

use aprilgrid::{
    TagFamily,
    detector::{DetectorParams, TagDetector},
};
use apriltag_image::image::ColorType;
use camera_intrinsic_calibration::{
    board::{Board, create_default_6x6_board},
    detected_points::{FeaturePoint, FrameFeature},
    io::write_report,
    types::{CalibParams, RvecTvec, ToRvecTvec},
    util::*,
    visualization::*,
};
use camera_intrinsic_model::{self as model, CameraModel, GenericModel, OpenCVModel5};

use image::{DynamicImage, RgbImage};
use model::model_from_json;
use tokio::{sync::watch, time::Instant};

pub struct CalibratedModel {
    model: GenericModel<f64>,
}
impl CalibratedModel {
    pub fn new() -> Self {
        let mut path = Path::new("/etc/cam0.json");
        if !path.exists() {
            path = Path::new("./cam0.json");
        }

        // Load the camera model
        let model = model_from_json(path.to_str().unwrap());

        let det = TagDetector::new(&TagFamily::T36H11, None);
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
}
impl Calibrator {
    /// Initialize a new calibrator
    pub fn new() -> Self {
        Self {
            det: TagDetector::new(&TagFamily::T36H11, None),
            board: create_default_6x6_board(),
            frame_feats: Vec::new(),
            cam_model: GenericModel::OpenCVModel5(OpenCVModel5::zeros()),
            start: Instant::now(),
        }
    }

    /// Process a frame
    pub fn step(&mut self, mut rx: watch::Receiver<Arc<Vec<u8>>>) -> usize {
        if self.frame_feats.len() < 200 {
            let mut frame_feat = None;
            while frame_feat.is_none() {
                if rx.has_changed().is_ok() && rx.has_changed().unwrap() {
                    let img = DynamicImage::ImageRgb8(
                        RgbImage::from_vec(1280, 720, rx.borrow_and_update().to_vec()).unwrap(),
                    );

                    frame_feat =
                        camera_intrinsic_calibration::data_loader::image_to_option_feature_frame(
                            &self.det,
                            &img,
                            &create_default_6x6_board(),
                            MIN_CORNERS,
                            self.start.elapsed().as_nanos() as i64,
                        );
                }
            }

            self.frame_feats.push(frame_feat.unwrap());
        }

        self.frame_feats.len()
    }

    /// Collect data on some frames
    //pub fn collect_data(&mut self, mut rx: watch::Receiver<Arc<Vec<u8>>>) {
    //    self.frame_feats.clear();

    //    let st = Instant::now();
    //    while self.frame_feats.len() < 200 {
    //        if rx.has_changed().is_ok() && rx.has_changed().unwrap() {
    //            if let Some(frame_feat) = self.step(
    //                ,
    //                st.elapsed().as_nanos().try_into().unwrap(),
    //            ) {
    //                self.frame_feats.push(frame_feat);
    //            }
    //        }
    //    }
    //}

    /// Calibrate
    pub fn calibrate(&mut self) {
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

        if calib_res.is_none() {
            error!("failed to calibrate camera");
        }

        self.frame_feats.clear();
    }
}
