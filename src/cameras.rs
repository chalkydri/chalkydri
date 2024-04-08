use std::{error::Error, future::Future, time::Instant};
use libcamera::{camera_manager::CameraManager, stream::StreamRole, properties};

pub fn load_cameras() -> Result<(), Box<dyn Error>> {

    let man = CameraManager::new()?;

    let cameras = man.cameras();

    for i in 0..cameras.len() {
        let cam = cameras.get(i)?;
        println!("{}", cam.id());
        cam.properties().get::<properties::Model>()
        cam.acquire().unwrap();
        cam.generate_configuration(&[StreamRole::Raw, StreamRole::ViewFinder]).unwrap();
    }

    Ok(())
}

pub struct Camera {
    inner: libcamera::camera::ActiveCamera,
}
impl Future for Camera {
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        self.inner;
    }
}

pub struct Frame {
    /// The time at which the frame was requested
    pub req_time: Instant,
    /// The time at which the frame was captured by the camera
    pub cap_time: Instant,
    /// The time at which the frame was gotten from the camera
    pub got_time: Instant,
    data: Vec<T>,
}

pub struct CamManager {
    cameras: Vec<Cam>,
}
impl CamManager {
    pub fn init() -> Self {
        let man = CameraManager::new()?;
    
        let cameras = man.cameras();
    
        for i in 0..cameras.len() {
            let cam = cameras.get(i)?;
            println!("{}", cam.id());
            cam.acquire().unwrap();
            cam.generate_configuration(&[StreamRole::Raw, StreamRole::ViewFinder]).unwrap();
        }
    
        Ok(())
    }

    pub async fn request_frame(&mut self) {
        Cam
    }
}
impl Future for CamManager {
    type Output = ;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        //
    }
}
