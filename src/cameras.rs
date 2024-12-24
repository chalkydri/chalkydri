use libcamera::{
    camera_manager::CameraManager, framebuffer::AsFrameBuffer, framebuffer_allocator::{FrameBuffer, FrameBufferAllocator}, framebuffer_map::MemoryMappedFrameBuffer, properties, request::{Request, ReuseFlag}, stream::StreamRole
};
use std::{error::Error, time::Duration};

pub async fn load_cameras(frame_tx: std::sync::mpsc::Sender<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    use libcamera::controls::*;

    let man = CameraManager::new()?;

    let cameras = man.cameras();

    for i in 0..cameras.len() {
        let cam = cameras.get(i).unwrap();
        println!("{}", cam.id());

        let mut cam = cam.acquire().unwrap();

        let mut alloc = FrameBufferAllocator::new(&cam);

        let configs = cam
            .generate_configuration(&[StreamRole::Raw, StreamRole::ViewFinder])
            .unwrap();
        let config = configs.get(0).unwrap();
        let stream = config.stream().unwrap();

        // Allocate some buffers
        let buffers = alloc
            .alloc(&stream)?
            .into_iter()
            .map(|buf| MemoryMappedFrameBuffer::new(buf).unwrap())
            .collect::<Vec<_>>();

        let reqs = buffers
            .into_iter()
            .enumerate()
            .map(|(i, buf)| -> Result<Request, Box<dyn Error>> {
                // Create the initial request
                let mut req = cam.create_request(Some(i as u64)).unwrap();

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

        let (tx, rx) = std::sync::mpsc::channel();
        cam.on_request_completed(move |req| {
            tx.send(req).unwrap();
        });

        cam.start(None)?;
        for req in reqs {
            cam.queue_request(req)?;
        }

        let properties::Model(model) = cam.properties().get::<properties::Model>()?;

            loop {
                let mut req = rx.recv_timeout(Duration::from_millis(300)).expect("camera request failed");
                let framebuffer: &MemoryMappedFrameBuffer<FrameBuffer> = req.buffer(&stream).unwrap();

                let planes = framebuffer.data();
                let frame_data = planes.get(0).unwrap();
                let bytes_used = framebuffer.metadata().unwrap().planes().get(0).unwrap().bytes_used as usize;

                let data = &frame_data[..bytes_used];
                let data_clone = Vec::from_iter(data.iter().cloned());
                frame_tx.send(data_clone).unwrap();

                req.reuse(ReuseFlag::REUSE_BUFFERS);
                cam.queue_request(req).unwrap();
            }
    }

    Ok(())
}

