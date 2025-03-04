/***
 * THIS FILE IS CURSED
 * PLES SEND HELP
 */

use futures_executor::LocalPool;
use futures_util::StreamExt;
use gstreamer::{
    Bin, Buffer, BusSyncReply, Caps, DeviceMonitor, Element, ElementFactory, FlowSuccess, Fraction,
    MessageView, Pipeline, Sample, SampleRef, State, Stream, Structure,
    bus::BusWatchGuard,
    glib::{ControlFlow, MainLoop, Type, Value, WeakRef, future_with_timeout},
    message::DeviceAdded,
    prelude::*,
};

use gstreamer_app::{AppSink, AppSinkCallbacks};
#[cfg(feature = "ntables")]
use minint::NtConn;
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{collections::HashMap, error::Error, sync::Arc};
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
    calibrators: Arc<Mutex<HashMap<String, Calibrator>>>,
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

        let calibrators: Arc<Mutex<HashMap<String, Calibrator>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let calibrators_ = calibrators.clone();
        bus.set_sync_handler(move |_, msg| {
            let calibrators = &calibrators_;

            // Upgrade the weak ref to work with the pipeline
            let pipeline = pipeline_.upgrade().unwrap();

            match msg.view() {
                MessageView::DeviceAdded(msg) => {
                    let dev = msg.device();
                    debug!("got a new device");

                    if let Some(cam_configs) = &config.cameras {
                        if let Some(cam_config) = cam_configs
                            .clone()
                            .iter()
                            .filter(|cam| cam.display_name == dev.display_name().to_string())
                            .next()
                        {
                            debug!("found a config");

                            // Create the camera source
                            let cam = dev.create_element(Some(&cam_config.name)).unwrap();

                            // The camera preprocessing part:
                            //   [src]> capsfilter -> queue -> tee -> ...

                            // Create the elements
                            let filter = ElementFactory::make("capsfilter")
                                .property("caps", &dev.caps().unwrap())
                                .build()
                                .unwrap();
                            //let queue = ElementFactory::make("queue").build().unwrap();
                            let tee = ElementFactory::make("tee").build().unwrap();

                            // Add them to the pipeline
                            pipeline.add_many([&cam, &filter, &tee]).unwrap();

                            // Link them
                            Element::link_many([&cam, &filter, &tee]).unwrap();

                            debug!("initializing calibrator");
                            let calibrator = Self::add_calib(&pipeline, &tee, cam_config.clone());

                            {
                                debug!("adding calibrator");
                                let mut calibrators = calibrators.blocking_lock();
                                (*calibrators).insert(cam_config.name.clone(), calibrator);
                                drop(calibrators);
                                debug!("dropped lock");
                            }

                            Self::add_subsys::<CApriltagsDetector>(
                                &pipeline,
                                &tee,
                                cam_config.clone(),
                                nt.clone(),
                            );
                        }
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

        Self {
            dev_mon,
            pipeline,
            calibrators,
        }
    }
    pub fn devices(&self) -> Vec<config::Camera> {
        //if self.pipeline.current_state() == State::Playing {
        //    self.pause();
        //}

        let mut devices = Vec::new();

        for dev in self.dev_mon.devices().iter() {
            let mut name = dev.name().to_string();
            //if dev.has_property("properties", Some(Type::PARAM_SPEC)) {
            //    name = dev
            //        .property::<Structure>("properties")
            //        .get::<String>("node.name")
            //        .unwrap();
            //}
            devices.push(config::Camera {
                name,
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
                subsystems: config::Subsystems {
                    capriltags: config::CAprilTagsSubsys {
                        enabled: false,
                        field_layout: None,
                        gamma: None,
                        field_layouts: HashMap::new(),
                    },
                    ml: config::MlSubsys { enabled: false },
                },
                calib: None,
            });
        }

        //if self.pipeline.current_state() == State::Paused {
        //    self.start();
        //}

        devices
    }
    pub async fn calib_step(&self, name: String) -> usize {
        self.calibrators().await.get_mut(&name).unwrap().step()
    }
    // gamma gamma=2.0 ! fpsdisplaysink ! videorate drop-only=true ! omxh264enc ! mpegtsenc !
    // rtspserversink port=1234

    pub(crate) fn add_subsys<S: Subsystem>(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
        nt: NtConn,
    ) {
        let target = format!("chalkydri::subsys::{}", S::NAME);

        debug!(target: &target, "initializing preproc pipeline chunk subsystem...");
        let (input, output) = S::preproc(cam_config.clone(), pipeline).unwrap();

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
        let cam_config = cam_config.clone();
        std::thread::spawn(move || {
            debug!("capriltags worker thread started");
            let nt = nt_;

            futures_executor::block_on(async move {
                debug!("initializing subsystem...");
                let mut subsys = S::init(cam_config).await.unwrap();

                debug!("starting subsystem...");
                subsys.process(nt, rx).await.unwrap();
            });
        });
    }

    pub(crate) fn add_calib(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
    ) -> Calibrator {
        let target = format!("chalkydri::camera::{}", cam_config.name);

        let valve = ElementFactory::make("valve")
            .property("drop", false)
            .build()
            .unwrap();
        let queue = ElementFactory::make("queue").build().unwrap();
        let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
        let filter = ElementFactory::make("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
                    .field("width", &1280)
                    .field("height", &720)
                    .field("format", "RGB")
                    .build(),
            )
            .build()
            .unwrap();
        let appsink = ElementFactory::make("appsink").build().unwrap();

        pipeline
            .add_many([&valve, &queue, &videoconvertscale, &filter, &appsink])
            .unwrap();
        Element::link_many([&cam, &valve, &queue, &videoconvertscale, &filter, &appsink]).unwrap();

        let appsink = appsink.dynamic_cast::<AppSink>().unwrap();

        let (tx, rx) = watch::channel(None);

        debug!(target: &target, "setting appsink callbacks...");
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().unwrap();
                    let buf = sample.buffer().unwrap();
                    tx.send(Some(buf.to_owned())).unwrap();

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        debug!("linked subsys junk");

        Calibrator::new(valve.downgrade(), rx)
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
    pub async fn calibrators(&self) -> MutexGuard<HashMap<String, Calibrator>> {
        self.calibrators.lock().await
    }
}
