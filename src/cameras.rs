use gstreamer_app::{glib, gst::FlowSuccess, AppSink, AppSinkCallbacks};
use gstreamer_base::gst::{self, Caps, Pipeline};
use gst::prelude::*;

#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc};
use tokio::sync::watch;

#[cfg(feature = "rerun")]
use crate::Rerun;
pub fn load_cameras(frame_tx: watch::Sender<Arc<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
    gst::init()?;

    let pipeline = gst::parse::launch(
        "libcamerasrc ! \
            video/x-raw,width=1280,height=720,framerate=60/1 ! \
            videoconvert ! \
            appsink"
    ).unwrap().dynamic_cast::<Pipeline>().unwrap();

    let appsink = pipeline.iterate_sinks().next().unwrap().unwrap().dynamic_cast::<AppSink>().unwrap();
    let caps = Caps::builder("video/x-raw").field("format", "RGB").build();
    appsink.set_caps(Some(&caps));

    let appsink_clone = appsink.clone();
    appsink.set_callbacks(AppSinkCallbacks::builder().new_sample(move |_| {
        let sample = appsink_clone.pull_sample().unwrap();
        let buf = sample.buffer().unwrap().map_readable().unwrap();

        frame_tx.send(Arc::new(buf.to_vec())).unwrap();

        Ok(FlowSuccess::Ok)
    }).build());

    let main_loop = glib::MainLoop::new(None, false);
    let main_loop_clone = main_loop.clone();
    let bus = pipeline.bus().unwrap();

    bus.connect_message(Some("error"), move |_, msg| match msg.view() {
        gst::MessageView::Error(err) => {
            let main_loop = &main_loop_clone;
            eprintln!(
                "Error received from element {:?}: {}",
                err.src().map(|s| s.path_string()),
                err.error()
            );
            eprintln!("Debugging information: {:?}", err.debug());
            main_loop.quit();
        }
        _ => unreachable!(),
    });
    bus.add_signal_watch();

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state.");

    main_loop.run();

    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state.");

    bus.remove_signal_watch();

//    let mut cw = CamWrapper::new(active_cam, cfgg, frame_tx);
//    cw.setup();
//    cw.run();

    Ok(())
}

//pub struct CamWrapper<'cam> {
//    cam: ActiveCamera<'cam>,
//    alloc: FrameBufferAllocator,
//    frame_tx: watch::Sender<Arc<Vec<u8>>>,
//    cam_tx: std::sync::mpsc::Sender<Request>,
//    cam_rx: std::sync::mpsc::Receiver<Request>,
//    configs: CameraConfiguration,
//}
//impl<'cam> CamWrapper<'cam> {
//    /// Wrap an [ActiveCamera]
//    pub fn new(
//        mut cam: ActiveCamera<'cam>,
//        mut cfgg: CameraConfiguration,
//        frame_tx: watch::Sender<Arc<Vec<u8>>>,
//    ) -> Self {
//        let alloc = FrameBufferAllocator::new(&cam);
//        cam.configure(&mut cfgg).unwrap();
//        let (cam_tx, cam_rx) = std::sync::mpsc::channel();
//        Self {
//            cam,
//            alloc,
//            cam_tx,
//            cam_rx,
//            frame_tx,
//            configs: cfgg,
//        }
//    }
//
//    /// Set up the camera and request the first frame
//    pub fn setup(&mut self) {
//        use libcamera::controls::*;
//        let stream = self.configs.get(0).unwrap();
//        let stream = stream.stream().unwrap();
//        // Allocate some buffers
//        let buffers = self
//            .alloc
//            .alloc(&stream)
//            .unwrap()
//            .into_iter()
//            .map(|buf| MemoryMappedFrameBuffer::new(buf).unwrap())
//            .collect::<Vec<_>>();
//        let reqs = buffers
//            .into_iter()
//            .enumerate()
//            .map(|(i, buf)| -> Result<Request, Box<dyn Error>> {
//                // Create the initial request
//                let mut req = self.cam.create_request(Some(i as u64)).unwrap();
//                // Set control values for the camera
//                {
//                    let ctrl = &mut req.controls_mut();
//                    // Autofocus
//                    (*ctrl).set(AfMode::Auto)?;
//                    (*ctrl).set(AfSpeed::Fast)?;
//                    //(*ctrl).set(AfRange::Full)?;
//                    // Autoexposure
//                    //(*ctrl).set(AeEnable(true))?;
//                    // TODO: make autoexposure constraint an option in the config UI
//                    // Maybe some logic to automatically set it based on lighting conditions?
//                    //(*ctrl).set(AeConstraintMode::ConstraintShadows)?;
//                    //(*ctrl).set(AeMeteringMode::MeteringCentreWeighted)?;
//                    //(*ctrl).set(FrameDuration(1000i64 / 60i64))?;
//                }
//                // Add buffer to the request
//                req.add_buffer(&stream, buf)?;
//                Ok(req)
//            })
//            .map(|x| x.unwrap())
//            .collect::<Vec<_>>();
//        let tx = self.cam_tx.clone();
//        self.cam.on_request_completed(move |req| {
//            tx.send(req).unwrap();
//        });
//        self.cam.start(None).unwrap();
//        for req in reqs {
//            self.cam.queue_request(req).unwrap();
//        }
//        //let properties::Model(_model) = self.cam.properties().get::<properties::Model>().unwrap();
//    }
//
//    /// Get a frame and request another
//    pub fn get_frame(&mut self) {
//        let stream = self.configs.get(0).unwrap().stream().unwrap();
//        let mut req = self
//            .cam_rx
//            .recv_timeout(Duration::from_millis(2000))
//            .expect("camera request failed");
//        let framebuffer: &MemoryMappedFrameBuffer<FrameBuffer> = req.buffer(&stream).unwrap();
//        let planes = framebuffer.data();
//        let y_plane = planes.get(0).unwrap();
//        let u_plane = planes.get(1).unwrap();
//        let v_plane = planes.get(2).unwrap();
//        let image = YuvPlanarImage {
//            width: 1920,
//            height: 1080,
//            y_plane,
//            u_plane,
//            v_plane,
//            y_stride: 1920,
//            u_stride: 960,
//            v_stride: 960,
//        };
//        let mut buff = vec![0u8; 6_220__800];
//        yuv420_to_rgb(
//            &image,
//            &mut buff,
//            5760,
//            YuvRange::Limited,
//            YuvStandardMatrix::Bt601,
//        )
//        .unwrap();
//        debug!("color converted. sending...");
//        self.frame_tx.send(Arc::new(buff.clone())).unwrap();
//        drop(buff);
//        req.reuse(ReuseFlag::REUSE_BUFFERS);
//        debug!("queueing another request");
//        self.cam.queue_request(req).unwrap();
//    }
//
//    /// Continously request frames until the end of time
//    pub fn run(mut self) {
//        loop {
//            self.get_frame();
//        }
//    }
//}
