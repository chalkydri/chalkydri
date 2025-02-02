use libcamera::{
    camera::{ActiveCamera, CameraConfiguration},
    camera_manager::CameraManager,
    framebuffer::AsFrameBuffer,
    framebuffer_allocator::{FrameBuffer, FrameBufferAllocator},
    framebuffer_map::MemoryMappedFrameBuffer,
    properties,
    request::{Request, ReuseFlag},
    stream::StreamRole
};

#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc, time::Duration};
use yuvutils_rs::{yuv420_to_rgb, YuvPlanarImage, YuvRange, YuvStandardMatrix};
use tokio::sync::watch;

#[cfg(feature = "rerun")]
use crate::Rerun;
pub fn load_cameras(frame_tx: watch::Sender<Arc<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
    let man = CameraManager::new()?;
    let cameras = man.cameras();
    // TODO: this must not crash the software
    assert!(cameras.len() > 0, "connect a camera");
    let cam = cameras.get(0).unwrap();
    info!("using camera '{}'", cam.id());
    let cfgg = cam
        .generate_configuration(&[StreamRole::VideoRecording])
        .unwrap();
    dbg!(&cfgg);

    let active_cam = cam.acquire().unwrap();
    let mut cw = CamWrapper::new(active_cam, cfgg, frame_tx);
    cw.setup();
    cw.run();

    Ok(())
}

pub struct CamWrapper<'cam> {
    cam: ActiveCamera<'cam>,
    alloc: FrameBufferAllocator,
    frame_tx: watch::Sender<Arc<Vec<u8>>>,
    cam_tx: std::sync::mpsc::Sender<Request>,
    cam_rx: std::sync::mpsc::Receiver<Request>,
    configs: CameraConfiguration
}
impl<'cam> CamWrapper<'cam> {
    /// Wrap an [ActiveCamera]
    pub fn new(
        mut cam: ActiveCamera<'cam>,
        mut cfgg: CameraConfiguration,
        frame_tx: watch::Sender<Arc<Vec<u8>>>,
    ) -> Self {
        let alloc = FrameBufferAllocator::new(&cam);
        cam.configure(&mut cfgg).unwrap();
        let (cam_tx, cam_rx) = std::sync::mpsc::channel();
        Self {
            cam,
            alloc,
            cam_tx,
            cam_rx,
            frame_tx,
            configs: cfgg,
        }
    }

    /// Set up the camera and request the first frame
    pub fn setup(&mut self) {
        use libcamera::controls::*;
        let stream = self.configs.get(0).unwrap();
        let stream = stream.stream().unwrap();
        // Allocate some buffers
        let buffers = self
            .alloc
            .alloc(&stream)
            .unwrap()
            .into_iter()
            .map(|buf| MemoryMappedFrameBuffer::new(buf).unwrap())
            .collect::<Vec<_>>();
        let reqs = buffers
            .into_iter()
            .enumerate()
            .map(|(i, buf)| -> Result<Request, Box<dyn Error>> {
                // Create the initial request
                let mut req = self.cam.create_request(Some(i as u64)).unwrap();
                // Set control values for the camera
                {
                    let ctrl = &mut req.controls_mut();
                    // Autofocus
                    (*ctrl).set(AfMode::Auto)?;
                    (*ctrl).set(AfSpeed::Fast)?;
                    (*ctrl).set(AfRange::Full)?;
                    // Autoexposure
                    (*ctrl).set(AeEnable(true))?;
                    // TODO: make autoexposure constraint an option in the config UI
                    // Maybe some logic to automatically set it based on lighting conditions?
                    (*ctrl).set(AeConstraintMode::ConstraintShadows)?;
                    (*ctrl).set(AeMeteringMode::MeteringCentreWeighted)?;
                    (*ctrl).set(FrameDuration(1000i64 / 60i64))?;
                }
                // Add buffer to the request
                req.add_buffer(&stream, buf)?;
                Ok(req)
            })
            .map(|x| x.unwrap())
            .collect::<Vec<_>>();
        let tx = self.cam_tx.clone();
        self.cam.on_request_completed(move |req| {
            tx.send(req).unwrap();
        });
        self.cam.start(None).unwrap();
        for req in reqs {
            self.cam.queue_request(req).unwrap();
        }
        let properties::Model(_model) = self.cam.properties().get::<properties::Model>().unwrap();
    }

    /// Get a frame and request another
    pub fn get_frame(&mut self) {
        let stream = self.configs.get(0).unwrap().stream().unwrap();
        let mut req = self
            .cam_rx
            .recv_timeout(Duration::from_millis(2000))
            .expect("camera request failed");
        let framebuffer: &MemoryMappedFrameBuffer<FrameBuffer> = req.buffer(&stream).unwrap();
        let planes = framebuffer.data();
        let y_plane = planes.get(0).unwrap();
        let u_plane = planes.get(1).unwrap();
        let v_plane = planes.get(2).unwrap();
        let image = YuvPlanarImage {
            width: 1920,
            height: 1080,
            y_plane,
            u_plane,
            v_plane,
            y_stride: 1920,
            u_stride: 960,
            v_stride: 960,
        };
        let mut buff = vec![0u8; 6_220__800];
        yuv420_to_rgb(
            &image,
            &mut buff,
            5760,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
        )
        .unwrap();
        debug!("color converted. sending...");
        self.frame_tx.send(Arc::new(buff.clone())).unwrap();
        drop(buff);
        req.reuse(ReuseFlag::REUSE_BUFFERS);
        debug!("queueing another request");
        self.cam.queue_request(req).unwrap();
    }

    /// Continously request frames until the end of time
    pub fn run(mut self) {
        loop {
            self.get_frame();
        }
    }
}
