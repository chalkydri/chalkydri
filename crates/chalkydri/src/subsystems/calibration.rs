use chalkydri_core::prelude::{Mutex, RwLock};
use cu_sensor_payloads::CuImage;
use cu29::prelude::*;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use image::{DynamicImage, GrayImage, Luma};

use tokio::time::Instant;

use crossbeam_channel::{Receiver, Sender, bounded};

pub static CALIB: LazyLock<RwLock<Option<Receiver<(DynamicImage, Duration)>>>> = LazyLock::new(|| RwLock::new(None));

/// A camera calibrator
pub struct Calibrator {
    start: Instant,
    tx: Sender<(DynamicImage, Duration)>,
}
impl Freezable for Calibrator {}
impl CuSinkTask for Calibrator {
    type Input<'m> = input_msg!((CuImage<Vec<u8>>, CuDuration));
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let (tx, rx) = bounded(1);

        *CALIB.write() = Some(rx);

        Ok(Self {
            start: Instant::now(),
            tx,
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

            let _ = self.tx.try_send((img, ts));
        }

        Ok(())
    }
}

