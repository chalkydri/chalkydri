use libcamera::{
    camera::{ActiveCamera, CameraConfiguration},
    camera_manager::CameraManager,
    framebuffer::AsFrameBuffer,
    framebuffer_allocator::{FrameBuffer, FrameBufferAllocator},
    framebuffer_map::MemoryMappedFrameBuffer,
    pixel_format::PixelFormat,
    properties,
    request::{Request, ReuseFlag},
    stream::StreamRole,
};
use std::{error::Error, time::Duration};

pub async fn load_cameras(
    frame_tx: std::sync::mpsc::Sender<Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    let man = CameraManager::new()?;

    let cameras = man.cameras();

    // TODO: this must not crash the software
    assert!(cameras.len() > 0, "connect a camera");

    let cam = cameras.get(0).unwrap();
    info!("using camera '{}'", cam.id());

    let active_cam = cam.acquire().unwrap();

    let mut cw = CamWrapper::new(active_cam, frame_tx);
    cw.setup();
    cw.run();

    Ok(())
}

pub struct CamWrapper<'cam> {
    cam: ActiveCamera<'cam>,
    alloc: FrameBufferAllocator,
    frame_tx: std::sync::mpsc::Sender<Vec<u8>>,
    cam_tx: std::sync::mpsc::Sender<Request>,
    cam_rx: std::sync::mpsc::Receiver<Request>,
    configs: CameraConfiguration,
}
impl<'cam> CamWrapper<'cam> {
    /// Wrap an [ActiveCamera]
    pub fn new(cam: ActiveCamera<'cam>, frame_tx: std::sync::mpsc::Sender<Vec<u8>>) -> Self {
        let alloc = FrameBufferAllocator::new(&cam);

        let mut configs = cam
            .generate_configuration(&[StreamRole::Raw, StreamRole::ViewFinder])
            .unwrap();

        configs
            .get_mut(0)
            .unwrap()
            .set_pixel_format(PixelFormat::new(
                u32::from_le_bytes([b'R', b'G', b'B', b'8']),
                0,
            ));

        let (cam_tx, cam_rx) = std::sync::mpsc::channel();

        Self {
            cam,
            alloc,
            frame_tx,
            cam_tx,
            cam_rx,
            configs,
        }
    }

    /// Set up the camera and request the first frame
    pub fn setup(&mut self) {
        use libcamera::controls::*;

        let stream = self.configs.get(0).unwrap().stream().unwrap();

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
            .recv_timeout(Duration::from_millis(300))
            .expect("camera request failed");
        let framebuffer: &MemoryMappedFrameBuffer<FrameBuffer> = req.buffer(&stream).unwrap();

        let planes = framebuffer.data();
        let frame_data = planes.get(0).unwrap();
        let bytes_used = framebuffer
            .metadata()
            .unwrap()
            .planes()
            .get(0)
            .unwrap()
            .bytes_used as usize;

        let data = &frame_data[..bytes_used];
        let data_clone = Vec::from_iter(data.iter().cloned());
        self.frame_tx.send(data_clone).unwrap();

        req.reuse(ReuseFlag::REUSE_BUFFERS);
        self.cam.queue_request(req).unwrap();
    }

    /// Continously request frames until the end of time
    pub fn run(mut self) {
        loop {
            self.get_frame();
        }
    }
}
