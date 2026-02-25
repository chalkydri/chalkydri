use chalkydri_core::prelude::{Mutex, RwLock};
use cu_sensor_payloads::CuImage;
use cu29::prelude::*;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use image::{DynamicImage, GrayImage, Luma};

use tokio::time::Instant;

pub static CALIB: LazyLock<Arc<Mutex<Option<(DynamicImage, Duration)>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

/// A camera calibrator
#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct Calibrator {
    #[reflect(ignore)]
    start: Instant,
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
            start: Instant::now(),
        })
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        //
        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, _clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        if let Some(img) = input.payload() {
            let ts = self.start.elapsed();
            let buf = img.0.as_image_buffer::<Luma<u8>>().expect("image buffer");
            let img = DynamicImage::ImageLuma8(
                GrayImage::from_vec(buf.width(), buf.height(), buf.to_vec()).unwrap(),
            );

            *CALIB.lock() = Some((img, ts));
        }

        Ok(())
    }
}
