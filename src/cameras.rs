use gstreamer::{
    Caps, DeviceMonitor, Element, ElementFactory, FlowSuccess, Fraction, MessageView, Pipeline,
    Sample, SampleRef, State, Stream,
    glib::{MainLoop, WeakRef},
    prelude::*,
};

use gstreamer_app::{AppSink, AppSinkCallbacks, app_sink::AppSinkStream};
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc};
use tokio::sync::{Mutex, MutexGuard, watch};

#[cfg(feature = "rerun")]
use crate::Rerun;
use crate::{
    Cfg,
    calibration::Calibrator,
    config::{self, CameraSettings, CfgFraction},
    subsys::capriltags,
};

#[derive(Clone)]
pub struct CameraCtx {
    cfgg: config::Camera,
    tee: WeakRef<Element>,
}

#[derive(Clone)]
pub struct CameraManager {
    dev_mon: DeviceMonitor,
    pipeline: Pipeline,
    pub main_loop: MainLoop,
    calibrator: Arc<Mutex<Calibrator>>,

    pub capriltags: Vec<watch::Receiver<Option<Vec<u8>>>>,
    cameras: Vec<CameraCtx>,
}
impl CameraManager {
    pub fn new() -> Self {
        gstreamer::assert_initialized();

        let dev_mon = DeviceMonitor::new();
        let caps = Caps::builder("video/x-raw").any_features().build();
        dev_mon
            .add_filter(Some("Video/Source"), Some(&caps))
            .unwrap();

        let pipeline = Pipeline::new();
        pipeline.set_async_handling(true);

        let main_loop = MainLoop::new(None, false);

        let calibrator = Arc::new(Mutex::new(Calibrator::new()));

        Self {
            dev_mon,
            pipeline,
            main_loop,
            calibrator,

            cameras: Vec::new(),
            capriltags: Vec::new(),
        }
    }
    pub fn devices(&self) -> Vec<config::Camera> {
        if self.pipeline.current_state() == State::Playing {
            self.pause();
        }

        let mut devices = Vec::new();

        self.dev_mon.start().unwrap();

        for dev in self.dev_mon.devices().iter() {
            devices.push(config::Camera {
                name: dev.name().to_string(),
                display_name: dev.display_name().to_string(),
                settings: None,
                possible_settings: Some(
                    dev.caps()
                        .unwrap()
                        .iter()
                        .map(|cap| {
                            let frame_rate = cap
                                .get::<Fraction>("framerate")
                                .unwrap_or_else(|_| Fraction::new(30, 1));
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
                        .collect(),
                ),
            });
        }

        self.dev_mon.stop();
        //if self.pipeline.current_state() == State::Paused {
        //    self.start();
        //}

        devices
    }
    // gamma gamma=2.0 ! fpsdisplaysink ! videorate drop-only=true ! omxh264enc ! mpegtsenc !
    // rtspserversink port=1234

    pub fn add_subsys(
        &self,
        func: impl Fn(&Pipeline) -> (Element, Element),
    ) -> Vec<watch::Receiver<Option<Vec<u8>>>> {
        let mut appsinks = Vec::new();

        for cam in &self.cameras {
            let (input, output) = func(&self.pipeline);

            cam.tee.upgrade().unwrap().link(&input).unwrap();

            let appsink = ElementFactory::make("appsink").build().unwrap();
            self.pipeline.add(&appsink).unwrap();
            output.link(&appsink).unwrap();
            let appsink = appsink.dynamic_cast::<AppSink>().unwrap();

            let (tx, rx) = watch::channel(None);

            let appsink_ = appsink.clone();
            appsink.set_callbacks(
                AppSinkCallbacks::builder()
                    .new_sample(move |_| {
                        let sample = appsink_.pull_sample().unwrap();
                        println!("{:?}", sample.info());
                        let buf = sample.buffer().unwrap().map_readable().unwrap();
                        tx.send(Some(buf.to_vec())).unwrap();

                        Ok(FlowSuccess::Ok)
                    })
                    .build(),
            );
            appsinks.push(rx);
        }

        appsinks
    }

    pub fn load_camera(&mut self, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
        // Get a copy of the global configuration
        let config = {
            let cfgg = Cfg.blocking_read();
            let ret = (*cfgg).clone();
            drop(cfgg);
            ret
        };

        let default_config = config::Camera {
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

        for cam_config in config.cameras {
            let cam_settings = cam_config.settings.clone().unwrap();

            self.dev_mon.start().unwrap();
            let devices = self.dev_mon.devices();
            if let Some(dev) = devices
                .iter()
                .filter(|cam| cam_config.name == cam.name())
                .next()
            {
                let cam = dev.create_element(None).unwrap();
                dbg!(cam.name());

                //let mut caps_ = dev.caps().clone().unwrap();
                //let mut caps = caps_.iter().filter(|cap| {
                //    cap.name().as_str() == "video/x-raw"
                //        && cap.get::<i32>("width").unwrap() == cam_settings.width as i32
                //});
                //dev.caps()
                //    .unwrap()
                //    .merge_structure(caps.next().unwrap().to_owned());
                //let caps = Caps::builder("video/x-raw")
                //    .field("width", &cam_settings.width)
                //    .field("height", &cam_settings.height)
                //    .field(
                //        "framerate",
                //        &Fraction::new(
                //            cam_settings.frame_rate.num as i32,
                //            cam_settings.frame_rate.den as i32,
                //        ),
                //    )
                //    .any_features()
                //    .build();

                let filter = ElementFactory::make("capsfilter")
                    .property("caps", &dev.caps().unwrap())
                    .build()
                    .unwrap();

                //cam.link_filtered(&convertscale, &caps).unwrap();
                let queue = ElementFactory::make("queue").build().unwrap();
                let tee = ElementFactory::make("tee").build().unwrap();

                self.pipeline
                    .add_many([&cam, &filter, &queue, &tee])
                    .unwrap();
                cam.link(&filter).unwrap();
                filter.link(&queue).unwrap();
                queue.link(&tee).unwrap();

                self.cameras.push(CameraCtx {
                    tee: tee.downgrade(),
                    cfgg: cam_config,
                });
            }
        }

        let capriltags_streams = self.add_subsys(|pipeline| {
            let gamma = ElementFactory::make("gamma")
                .property("gamma", &config.subsystems.capriltags.gamma.unwrap_or(1.0))
                .build()
                .unwrap();
            let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
            let filter = ElementFactory::make("capsfilter")
                .property(
                    "caps",
                    &Caps::builder("video/x-raw")
                        .field("width", &1280)
                        .field("height", &720)
                        .field("format", "GRAY8")
                        .build(),
                )
                .build()
                .unwrap();
            pipeline
                .add_many([&gamma, &videoconvertscale, &filter])
                .unwrap();

            gamma.link(&videoconvertscale).unwrap();
            videoconvertscale.link(&filter).unwrap();
            (gamma, filter)
        });

        for (i, stream) in capriltags_streams.iter().enumerate() {
            let mut rx = stream.clone();
            tokio::spawn(async move {
                loop {
                    rx.changed().await.unwrap();
                    let buf = rx.borrow_and_update().clone().unwrap();
                    println!("{i}: {:?}", buf.get(0..10));
                }
            });
        }

        self.capriltags = capriltags_streams;

        Ok(())
    }
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let main_loop = self.main_loop.clone();

        // Define the event loop or something?
        self.pipeline
            .bus()
            .unwrap()
            .connect_message(Some("error"), move |_, msg| match msg.view() {
                MessageView::Error(err) => {
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
        self.main_loop.run();

        Ok(())
    }

    pub fn start(&self) {
        //trace!("waiting for pipeline to be ready");
        //while self.pipeline.current_state() != State::Ready {}
        //trace!("pipeline ready!");

        // Start the pipeline
        self.pipeline.set_state(State::Playing).unwrap();
        //.expect("Unable to set the pipeline to the `Playing` state.");
    }
    pub fn pause(&self) {
        self.pipeline
            .set_state(State::Paused)
            .expect("Unable to set the pipeline to the `Null` state.");
    }
    pub fn stop(&self) {
        self.pause();
        self.pipeline
            .remove_many(
                self.pipeline
                    .iterate_elements()
                    .into_iter()
                    .map(|x| x.unwrap()),
            )
            .unwrap();
    }
    pub async fn calibrator(&self) -> MutexGuard<Calibrator> {
        self.calibrator.lock().await
    }
}
