use chalkydri_core::prelude::{Mutex, RwLock};
use cu_sensor_payloads::CuImage;
use cu29::prelude::*;
use std::{cell::Cell, collections::HashMap, path::Path, sync::Arc, time::Duration};

use aprilgrid::{TagFamily, detector::TagDetector};
use camera_intrinsic_calibration::{
    board::{Board, create_default_6x6_board},
    detected_points::FrameFeature,
    types::CalibParams,
    util::*,
};
use camera_intrinsic_model::{GenericModel, OpenCVModel5};
use cu29::cutask::CuTask;
use image::{DynamicImage, GrayImage, Luma, RgbImage};

use gstreamer::{
    Buffer, Element,
    glib::{WeakRef, object::ObjectExt},
};
use tokio::{sync::watch, time::Instant};

pub struct CalibratedModel {
    model: GenericModel<f64>,
}
impl CalibratedModel {
    pub fn from_str(calib: String) -> Self {
        // Load the camera model
        let model = serde_json::from_str(&calib).unwrap();

        Self { model }
    }

    pub const fn inner_model(&self) -> GenericModel<f64> {
        self.model
    }
}

const MIN_CORNERS: usize = 24;
pub static CALIB_RESULT: Mutex<Option<CalibratedModel>> = Mutex::new(None);

/// A camera calibrator
pub struct Calibrator {
    det: TagDetector,
    board: Board,
    frame_feats: Vec<FrameFeature>,
    cam_model: GenericModel<f64>,
    start: Instant,
}
impl Calibrator {
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
            tracing::error!("failed to calibrate camera");
            None
        } else {
            Some(calib_res.unwrap().0)
        }
    }
}
impl Freezable for Calibrator {}
impl CuSinkTask for Calibrator {
    type Input<'m> = input_msg!((CuImage<Vec<u8>>, CuDuration));
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            det: TagDetector::new(&TagFamily::T36H11, None),
            board: create_default_6x6_board(),
            frame_feats: Vec::new(),
            cam_model: GenericModel::OpenCVModel5(OpenCVModel5::zeros()),
            start: Instant::now(),
        })
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, _clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        if self.frame_feats.len() < 200 {
            tracing::debug!("got frame");
            //valve.set_property("drop", true);
            if let Some(img) = input.payload() {
                let buf = img.0.as_image_buffer::<Luma<u8>>().expect("image buffer");
                let img = DynamicImage::ImageLuma8(
                    GrayImage::from_vec(buf.width(), buf.height(), buf.to_vec()).unwrap(),
                );

                if let Some(frame_feat) =
                    camera_intrinsic_calibration::data_loader::image_to_option_feature_frame(
                        &self.det,
                        &img,
                        &create_default_6x6_board(),
                        MIN_CORNERS,
                        self.start.elapsed().as_nanos() as i64,
                    )
                {
                    self.frame_feats.push(frame_feat);
                    println!("     > {}/200", self.frame_feats.len());
                }
            }
        } else {
            *CALIB_RESULT.lock() = Some(CalibratedModel {
                model: self.calibrate().unwrap(),
            });
        }

        Ok(())
    }
}
