/***
 * THIS FILE IS CURSED
 * PLES SEND HELP
 */

use futures_executor::LocalPool;
use futures_util::StreamExt;
use gstreamer::{
    Bin, Buffer, BusSyncReply, Caps, DeviceMonitor, Element, ElementFactory, FlowSuccess, Fraction,
    MessageView, Pipeline, Sample, SampleRef, State, Stream,
    bus::BusWatchGuard,
    glib::{ControlFlow, MainLoop, Value, WeakRef},
    message::DeviceAdded,
    prelude::*,
};

use gstreamer_app::{AppSink, AppSinkCallbacks};
#[cfg(feature = "ntables")]
use minint::NtConn;
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc};
use tokio::{
    sync::{Mutex, MutexGuard, watch},
    task::LocalSet,
    time::Instant,
};

#[cfg(feature = "rerun")]
use crate::Rerun;
use crate::{
    Cfg,
    calibration::Calibrator,
    config::{self, CAprilTagsSubsys, CameraSettings, CfgFraction},
    subsys::capriltags::CApriltagsDetector,
    subsystem::{SubsysCtx, Subsystem},
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
    calibrator: Arc<Mutex<Calibrator>>,
}
impl CameraManager {
    pub async fn new(#[cfg(feature = "ntables")] nt: NtConn) -> Self {
        // Make sure gstreamer is initialized
        gstreamer::assert_initialized();

        // Get a copy of the global configuration
        let config = {
            let cfgg = Cfg.read().await;
            let ret = (*cfgg).clone();
            drop(cfgg);
            ret
        };

        // Create a device monitor to watch for new devices
        let dev_mon = DeviceMonitor::new();
        let caps = Caps::builder("video/x-raw").any_features().build();
        dev_mon
            .add_filter(Some("Video/Source"), Some(&caps))
            .unwrap();

        // Create the pipeline
        let pipeline = Pipeline::new();

        let bus = dev_mon.bus();

        // Create weak ref to pipeline we can give away
        let pipeline_ = pipeline.downgrade();

        bus.set_sync_handler(move |_, msg| {
            // Upgrade the weak ref to work with the pipeline
            let pipeline = pipeline_.upgrade().unwrap();

            match msg.view() {
                MessageView::DeviceAdded(msg) => {
                    let dev = msg.device();
                    debug!("got a new device");

                    if let Some(cam_config) = config
                        .cameras
                        .clone()
                        .iter()
                        .filter(|cam| cam.display_name == dev.display_name().to_string())
                        .next()
                    {
                        debug!("found a config");

                        // Create the camera source
                        let cam = dev.create_element(Some(dev.name().as_str())).unwrap();

                        // The camera preprocessing part:
                        //   [src]> capsfilter -> queue -> tee -> ...

                        // Create the elements
                        let filter = ElementFactory::make("capsfilter")
                            .property("caps", &dev.caps().unwrap())
                            .build()
                            .unwrap();
                        let queue = ElementFactory::make("queue").build().unwrap();
                        let tee = ElementFactory::make("tee").build().unwrap();

                        // Add them to the pipeline
                        pipeline.add_many([&cam, &filter, &queue, &tee]).unwrap();

                        // Link them
                        Element::link_many([&cam, &filter, &queue, &tee]).unwrap();

                        let cam_config_ = cam_config.clone();

                        // Copy the NT conn to give away
                        #[cfg(feature = "ntables")]
                        let nt_ = nt.clone();

                        Self::add_subsys::<CApriltagsDetector>(
                            &pipeline,
                            &tee,
                            cam_config.clone(),
                            nt.clone(),
                        );
                    }
                }
                _ => {}
            }

            BusSyncReply::Pass
        });

        // Start the device monitor
        dev_mon.start().unwrap();

        // Start the pipeline
        pipeline.set_state(State::Playing).unwrap();

        // Get the pipeline's bus
        let bus = pipeline.bus().unwrap();
        // Hook up event handler for the pipeline
        bus.set_sync_handler(|_, _| BusSyncReply::Pass);

        let calibrator = Arc::new(Mutex::new(Calibrator::new()));

        Self {
            dev_mon,
            pipeline,
            calibrator,
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
        if self.pipeline.current_state() == State::Paused {
            self.start();
        }

        devices
    }
    pub async fn calib_step(&self) -> usize {
        //self.calibrator().await.step(self.capriltags.clone())
        0
    }
    // gamma gamma=2.0 ! fpsdisplaysink ! videorate drop-only=true ! omxh264enc ! mpegtsenc !
    // rtspserversink port=1234

    pub(crate) fn add_subsys<S: Subsystem>(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
        nt: NtConn,
    ) -> SubsysCtx {
        let target = format!("chalkydri::subsys::{}", S::NAME);

        debug!(target: &target, "initializing preproc pipeline chunk subsystem...");
        let (input, output) = S::preproc(config.clone(), pipeline).unwrap();

        let appsink = ElementFactory::make("appsink").build().unwrap();
        pipeline.add(&appsink).unwrap();

        debug!(target: &target, "linking preproc pipeline chunk...");
        cam.link(&input).unwrap();
        output.link(&appsink).unwrap();

        let appsink = appsink.dynamic_cast::<AppSink>().unwrap();

        let (tx, rx) = watch::channel(None);

        let appsink_ = appsink.clone();

        debug!(target: &target, "setting appsink callbacks...");
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |_| {
                    let sample = appsink_.pull_sample().unwrap();
                    let buf = sample.buffer().unwrap();
                    tx.send(Some(buf.to_owned())).unwrap();

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        debug!("linked subsys junk");

        let nt_ = nt.clone();
        let config = config.clone();
        let cam_config = cam_config.clone();
        std::thread::spawn(move || {
            let nt = nt_;
            let mut local = LocalPool::new();

            local.run_until(async move {
                let mut subsys = S::init(cam_config, config).await.unwrap();
                subsys.process(nt, rx).await.unwrap();
            });
        });

        SubsysCtx {}
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

                let caps = Caps::builder("video/x-raw")
                    .field("width", &cam_settings.width)
                    .field("height", &cam_settings.height)
                    .field(
                        "framerate",
                        &Fraction::new(
                            cam_settings.frame_rate.num as i32,
                            cam_settings.frame_rate.den as i32,
                        ),
                    )
                    .any_features()
                    .build();

                let filter = ElementFactory::make("capsfilter")
                    .property("caps", &caps)
                    .build()
                    .unwrap();

                //cam.link_filtered(&convertscale, &caps).unwrap();
                //let queue = ElementFactory::make("queue").build().unwrap();
                let tee = ElementFactory::make("tee").build().unwrap();

                self.pipeline.add_many([&cam, &filter, &tee]).unwrap();
                cam.link(&filter).unwrap();
                //filter.link(&queue).unwrap();
                filter.link(&tee).unwrap();

                //self.cameras.push(CameraCtx {
                //    tee: tee.downgrade(),
                //    cfgg: cam_config,
                //});
            }
        }

        //let capriltags_streams = self.add_subsys(|pipeline| {
        //    let gamma = ElementFactory::make("gamma")
        //        .property("gamma", &config.subsystems.capriltags.gamma.unwrap_or(1.0))
        //        .build()
        //        .unwrap();
        //    let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
        //    let filter = ElementFactory::make("capsfilter")
        //        .property(
        //            "caps",
        //            &Caps::builder("video/x-raw")
        //                .field("width", &1280)
        //                .field("height", &720)
        //                .field("format", "GRAY8")
        //                .build(),
        //        )
        //        .build()
        //        .unwrap();
        //    pipeline
        //        .add_many([&gamma, &videoconvertscale, &filter])
        //        .unwrap();

        //    gamma.link(&videoconvertscale).unwrap();
        //    videoconvertscale.link(&filter).unwrap();
        //    (gamma, filter)
        //});

        //for (i, stream) in capriltags_streams.iter().enumerate() {
        //    let mut rx = stream.clone();
        //    tokio::spawn(async move {
        //        loop {
        //            rx.changed().await.unwrap();
        //            let buf = rx.borrow_and_update().clone().unwrap();
        //            println!("{i}: {:?}", buf.get(0..10));
        //        }
        //    });
        //}

        //self.capriltags = capriltags_streams;

        Ok(())
    }
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
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
                }
                _ => unimplemented!(),
            });

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
