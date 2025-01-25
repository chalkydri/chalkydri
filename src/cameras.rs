use nokhwa::{
    pixel_format::RgbFormat,
    utils::{ApiBackend, RequestedFormat, RequestedFormatType},
    Camera,
};
use rerun::RecordingStream;
use std::{error::Error, sync::Arc};
use tokio::sync::watch;

pub fn load_cameras(
    frame_tx: watch::Sender<Arc<Vec<u8>>>,
    rr: RecordingStream,
) -> Result<(), Box<dyn Error>> {
    let cams = nokhwa::query(ApiBackend::Auto).unwrap();
    for cam in cams {
        let rr = rr.clone();
        let frame_tx = frame_tx.clone();
        std::thread::spawn(move || {
            if let Ok(cam) = Camera::new(
                cam.index().clone(),
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
            ) {
                dbg!(
                    cam.index(),
                    cam.info().human_name(),
                    cam.info().description(),
                    cam.info().misc()
                );
                info!("{}", cam.info().human_name());

                let mut cw = CamWrapper::new(cam, rr, frame_tx);
                cw.setup();
                cw.run();
            }
        });
    }

    Ok(())
}

pub struct CamWrapper {
    cam: Camera,
    rr: RecordingStream,
    frame_tx: watch::Sender<Arc<Vec<u8>>>,
}
impl CamWrapper {
    /// Wrap an [ActiveCamera]
    pub fn new(cam: Camera, rr: RecordingStream, frame_tx: watch::Sender<Arc<Vec<u8>>>) -> Self {
        Self { cam, rr, frame_tx }
    }

    /// Set up the camera and request the first frame
    pub fn setup(&mut self) {
        self.cam.open_stream().unwrap();
    }

    /// Get a frame and request another
    pub fn get_frame(&mut self) {
        let frame = self.cam.frame().unwrap();
        self.rr
            .log(
                "/images",
                &rerun::EncodedImage::new(frame.buffer().to_vec()),
            )
            .unwrap();
        let buff = frame.decode_image::<RgbFormat>().unwrap().to_vec();
        self.frame_tx.send(buff.into()).unwrap();
    }

    /// Continously request frames until the end of time
    pub fn run(mut self) {
        loop {
            self.get_frame();
        }
    }
}
