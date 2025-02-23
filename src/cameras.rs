use gst::prelude::*;
use gstreamer_app::{
    glib::MainLoop,
    gst::{DeviceMonitor, ElementFactory, FlowSuccess, Fraction, State, Structure},
    AppSink, AppSinkCallbacks,
};
use gstreamer_base::gst::{self, Caps, Pipeline};

#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc};
use tokio::sync::{watch, Mutex, MutexGuard};

#[cfg(feature = "rerun")]
use crate::Rerun;
use crate::{
    calibration::Calibrator,
    config::{self, CameraSettings, CfgFraction},
    Cfg,
};

#[derive(Clone)]
pub struct CameraManager {
    dev_mon: DeviceMonitor,
    pipeline: Pipeline,
    main_loop: MainLoop,
    calibrator: Arc<Mutex<Calibrator>>,
}
impl CameraManager {
    pub fn new() -> Self {
        gst::assert_initialized();

        let dev_mon = DeviceMonitor::new();
        let caps = Caps::builder("video/x-raw").any_features().build();
        dev_mon.add_filter(Some("Video/Source"), Some(&caps)).unwrap();
        dev_mon.start().unwrap();

        let pipeline = Pipeline::new();

        let main_loop = MainLoop::new(None, false);

        let calibrator = Arc::new(Mutex::new(Calibrator::new()));

        Self {
            dev_mon,
            pipeline,
            main_loop,
            calibrator,
        }
    }
    pub fn devices(&self) -> Vec<config::Camera> {
        if self.pipeline.current_state() == State::Playing {
            self.pause();
        }

        let mut devices = Vec::new();

        self.dev_mon.start().unwrap();

        for dev in self.dev_mon
            .devices()
            .iter() {
            devices.push(config::Camera {
                name: dev.name().to_string(),
                display_name: dev.display_name().to_string(),
                settings: None,
                possible_settings: Some(dev
                    .caps()
                    .unwrap()
                    .iter()
                    .map(|cap| {
                        let frame_rate = cap.get::<Fraction>("framerate").unwrap_or_else(|_| Fraction::new(30, 1));
                        CameraSettings {
                            width: cap.get::<i32>("width").unwrap() as u32,
                            height: cap.get::<i32>("height").unwrap() as u32,
                            frame_rate: CfgFraction {
                                num: frame_rate.numer() as u32,
                                den: frame_rate.denom() as u32,
                            },
                            gamma: None,
                        }
                    })
                    .collect()),
            });
        }

        self.dev_mon.stop();

        self.start();

        devices
    }
    // gamma gamma=2.0 ! fpsdisplaysink ! videorate drop-only=true ! omxh264enc ! mpegtsenc !
    // rtspserversink port=1234
    pub fn load_camera(
        &mut self,
        width: u32,
        height: u32,
        frame_tx: watch::Sender<Arc<Vec<u8>>>,
    ) -> Result<(), Box<dyn Error>> {
        let cam_settings = {
            let cfgg = Cfg.blocking_read();
            let ret = (*cfgg).clone();
            drop(cfgg);
            ret
        };
        let config = config::Camera {
            name: String::new(),
            display_name: String::new(),
            settings: Some(CameraSettings {
                width,
                height,
                gamma: None,
                frame_rate: CfgFraction { num: 50, den: 1 },
            }),
            possible_settings: None,
        };
        let cam_settings = cam_settings.cameras.first().unwrap_or(&config);
        let cam_settings = cam_settings.settings.clone().unwrap();

        let devices = self.dev_mon.devices();
        let dev = devices.front().unwrap();

        let elem = dev.create_element(None).unwrap();

        let filter = ElementFactory::make("capsfilter").property("caps", dev.caps().iter().next().unwrap()).build().unwrap();

        let convertscale = ElementFactory::make("videoconvertscale").build().unwrap();

        let appsink = ElementFactory::make("appsink").build().unwrap();

        self.pipeline.add_many([&elem, &convertscale, &filter, &appsink]).unwrap();
        elem.link_filtered(&convertscale, &Caps::builder("video/x-raw").field("width", 1280).field("height", 720).build()).unwrap();
        convertscale.link(&appsink).unwrap();

        // Parse pipeline description to create pipeline
        //self.pipeline = gst::parse::launch(&format!(
        //    "libcamerasrc ! \
        //    capsfilter name=caps_filter caps=video/x-raw,width={},height={} ! \
        //    videoconvertscale ! \
        //    appsink",
        //    //cam_settings.gamma.unwrap_or(1.0),
        //    cam_settings.width,
        //    cam_settings.height,
        //    //cam_settings.frame_rate.num,
        //    //cam_settings.frame_rate.den,
        //))
        //.unwrap()
        //.dynamic_cast::<Pipeline>()
        //.unwrap();

        // Get the sink
        let appsink = appsink.dynamic_cast::<AppSink>()
            .unwrap();

        // Force conversion to RGB pixel format
        let caps = Caps::builder("video/x-raw").field("format", "RGB").build();
        appsink.set_caps(Some(&caps));

        // Register a callback to handle new samples
        let appsink_clone = appsink.clone();
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |_| {
                    let sample = appsink_clone.pull_sample().unwrap();
                    let buf = sample.buffer().unwrap().map_readable().unwrap();

                    frame_tx.send(Arc::new(buf.to_vec())).unwrap();

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        let main_loop = self.main_loop.clone();

        // Define the event loop or something?
        self.pipeline
            .bus()
            .unwrap()
            .connect_message(Some("error"), move |_, msg| match msg.view() {
                gst::MessageView::Error(err) => {
                    error!(
                        "error received from element {:?}: {}",
                        err.src().map(|s| s.path_string()),
                        err.error()
                    );
                    debug!("{:?}", err.debug());

                    // Kill event loop
                    main_loop.quit();
                }
                _ => unimplemented!(),
            });


        self.start();

        self.main_loop.run();

        Ok(())
    }
    pub fn start(&self) {

        // Start the pipeline
        self.pipeline
            .set_state(State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state.");
    }
    pub fn pause(&self) {
        self.pipeline
            .set_state(gst::State::Paused)
            .expect("Unable to set the pipeline to the `Null` state.");
    }
    pub fn stop(&self) {
        //self.pipeline
        //    .remove_many(
        //        self.pipeline
        //            .iterate_elements()
        //            .into_iter()
        //            .map(|x| x.unwrap()),
        //    )
        //    .unwrap();
    }
    pub async fn calibrator(&self) -> MutexGuard<Calibrator> {
        self.calibrator.lock().await
    }
}
