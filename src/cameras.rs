/***
 * THIS FILE IS CURSED
 * PLES SEND HELP
 */

use actix_web::web::Bytes;
use futures_core::Stream;
use futures_executor::LocalPool;
use futures_util::StreamExt;
use gstreamer::{
    Bin, Buffer, BusSyncReply, Caps, CapsFeatures, DeviceMonitor, DeviceProvider,
    DeviceProviderFactory, Element, ElementFactory, FlowSuccess, Fraction, MessageView, Pipeline,
    Sample, SampleRef, State,
    bus::BusWatchGuard,
    glib::{
        ControlFlow, MainLoop, ParamSpecBoxed, Type, Value, WeakRef, future_with_timeout,
        property::PropertyGet,
    },
    message::DeviceAdded,
    prelude::*,
    subclass::prelude::DeviceProviderImpl,
};

use gstreamer_app::{AppSink, AppSinkCallbacks};
use minint::NtConn;
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{collections::HashMap, error::Error, pin::Pin, sync::Arc, task::Poll, time::Duration};
use tokio::{
    sync::{Mutex, MutexGuard, RwLock, watch},
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
pub struct MjpegStream {
    rx: watch::Receiver<Option<Buffer>>,
}
impl Stream for MjpegStream {
    type Item = Result<Bytes, Box<dyn Error>>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        while !self.rx.has_changed().unwrap() {}

        info!("working!!!");
        let mut bytes = Vec::new();
        bytes.clear();
        if let Some(frame) = self.get_mut().rx.borrow_and_update().as_deref() {
            bytes.extend_from_slice(
                &[
                    b"--frame\r\nContent-Length: ",
                    frame.size().to_string().as_bytes(),
                    b"\r\nContent-Type: image/jpeg\r\n\r\n",
                ]
                .concat(),
            );
            bytes.extend_from_slice(frame.map_readable().unwrap().as_slice());
        }
        Poll::Ready(Some(Ok(bytes.into())))
    }
}

#[derive(Clone)]
pub struct CameraManager {
    //dev_mon: DeviceMonitor,
    dev_prov: DeviceProvider,
    pipeline: Pipeline,
    calibrators: Arc<Mutex<HashMap<String, Calibrator>>>,
    mjpeg_streams: Arc<Mutex<HashMap<String, MjpegStream>>>,
}
impl CameraManager {
    pub async fn new(nt: NtConn) -> Self {
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
        let dev_prov = DeviceProviderFactory::find("libcameraprovider")
            .unwrap()
            .load()
            .unwrap()
            .get()
            .unwrap();
        //let dev_mon = DeviceMonitor::new();
        //let caps = Caps::builder("video/x-raw").any_features().build();
        //dev_mon
        //    .add_filter(Some("Video/Source"), Some(&caps))
        //    .unwrap();

        // Create the pipeline
        let pipeline = Pipeline::new();
        let mut pipelines = Arc::new(RwLock::new(Vec::new()));

        //let bus = dev_mon.bus();
        let bus = dev_prov.bus();

        // Create weak ref to pipeline we can give away
        let pipeline_ = pipeline.downgrade();

        let calibrators: Arc<Mutex<HashMap<String, Calibrator>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let mjpeg_streams: Arc<Mutex<HashMap<String, MjpegStream>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let calibrators_ = calibrators.clone();
        let mjpeg_streams_ = mjpeg_streams.clone();
        let pipelines_ = pipelines.clone();
        bus.set_sync_handler(move |_, msg| {
            let calibrators = &calibrators_;
            let mjpeg_streams = &mjpeg_streams_;
            let pipelines = &pipelines_;

            // Upgrade the weak ref to work with the pipeline
            let pipeline = pipeline_.upgrade().unwrap();
            let pipeline = Pipeline::new();

            match msg.view() {
                MessageView::DeviceAdded(msg) => {
                    let dev = msg.device();
                    debug!("got a new device");

                    if let Some(cam_configs) = &config.cameras {
                        if let Some(cam_config) = cam_configs
                            .clone()
                            .iter()
                            .filter(|cam| cam.id == dev.display_name().to_string())
                            .next()
                        {
                            debug!("found a config");

                            // Create the camera source
                            let cam = dev.create_element(None).unwrap();
                            dbg!(cam.list_properties());

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
                                let mut calibrators =
                                    tokio::task::block_in_place(|| calibrators.blocking_lock());
                                (*calibrators).insert(cam_config.id.clone(), calibrator);
                                drop(calibrators);
                                debug!("dropped lock");
                            }

                            debug!("initializing mjpeg stream");
                            let mjpeg_stream = Self::add_mjpeg(&pipeline, &tee, cam_config.clone());

                            {
                                debug!("adding mjpeg stream");
                                let mut mjpeg_streams =
                                    tokio::task::block_in_place(|| mjpeg_streams.blocking_lock());
                                (*mjpeg_streams).insert(cam_config.id.clone(), mjpeg_stream);
                                drop(mjpeg_streams);
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
            tokio::task::block_in_place(|| pipelines.blocking_write()).push(pipeline);

            BusSyncReply::Pass
        });

        // Start the device monitor
        dev_prov.start().unwrap();

        for pipeline in pipelines.read().await.clone() {
            // Start the pipeline
            pipeline.set_state(State::Playing).unwrap();

            // Get the pipeline's bus
            let bus = pipeline.bus().unwrap();
            // Hook up event handler for the pipeline
            bus.set_sync_handler(|_, _| BusSyncReply::Pass);
        }

        Self {
            //dev_mon,
            dev_prov,
            pipeline,
            calibrators,
            mjpeg_streams,
        }
    }

    /// List connected cameras
    pub fn devices(&self) -> Vec<config::Camera> {
        let mut devices = Vec::new();

        for dev in self.dev_prov.devices().iter() {
            devices.push(config::Camera {
                id: dev.display_name().to_string(),
                name: String::new(),
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
                    },
                    ml: config::MlSubsys { enabled: false },
                },
                calib: None,
            });
        }

        devices
    }
    pub async fn calib_step(&self, name: String) -> usize {
        self.calibrators().await.get_mut(&name).unwrap().step()
    }
    pub async fn mjpeg_stream(&self, name: String) -> MjpegStream {
        self.mjpeg_streams().await.get(&name).unwrap().clone()
    }

    /// Add [`subsystem`](Subsystem) to pipeline
    pub(crate) fn add_subsys<S: Subsystem>(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
        nt: NtConn,
    ) {
        let target = format!("chalkydri::subsys::{}", S::NAME);

        debug!(target: &target, "initializing preproc pipeline chunk subsystem...");
        let (input, output) = S::preproc(cam_config.clone(), pipeline).unwrap();

        let queue = ElementFactory::make("queue").build().unwrap();
        let appsink = ElementFactory::make("appsink").build().unwrap();
        pipeline.add_many([&queue, &appsink]).unwrap();

        debug!(target: &target, "linking preproc pipeline chunk...");
        Element::link_many([&cam, &queue, &input]).unwrap();
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
        let target = format!("chalkydri::camera::{}", cam_config.id);

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
                    while let Err(err) = tx.send(Some(buf.to_owned())) {
                        error!("error sending frame: {err:?}");
                    }

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        debug!("linked subsys junk");

        Calibrator::new(valve.downgrade(), rx)
    }

    // gamma gamma=2.0 ! fpsdisplaysink ! videorate drop-only=true ! omxh264enc ! mpegtsenc !
    pub(crate) fn add_mjpeg(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
    ) -> MjpegStream {
        let target = format!("chalkydri::camera::{}", cam_config.id);

        let valve = ElementFactory::make("valve")
            .property("drop", false)
            .build()
            .unwrap();
        let queue = ElementFactory::make("queue").build().unwrap();
        let videoconvertscale = ElementFactory::make("videoconvertscale")
            .property_from_str("method", "nearest-neighbour")
            .build()
            .unwrap();
        let filter = ElementFactory::make("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
                    .field("width", &720)
                    .field("height", &480)
                    .field("format", "RGB")
                    .build(),
            )
            .build()
            .unwrap();
        let jpegenc = ElementFactory::make("jpegenc")
            .property("quality", &25)
            .build()
            .unwrap();
        let appsink = ElementFactory::make("appsink").build().unwrap();

        pipeline
            .add_many([
                &valve,
                &queue,
                &videoconvertscale,
                &filter,
                &jpegenc,
                &appsink,
            ])
            .unwrap();
        Element::link_many([
            &cam,
            &valve,
            &queue,
            &videoconvertscale,
            &filter,
            &jpegenc,
            &appsink,
        ])
        .unwrap();

        let appsink = appsink.dynamic_cast::<AppSink>().unwrap();

        let (tx, rx) = watch::channel(None);

        debug!(target: &target, "setting appsink callbacks...");
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().unwrap();
                    let buf = sample.buffer().unwrap();
                    while let Err(err) = tx.send(Some(buf.to_owned())) {
                        error!("error sending frame: {err:?}");
                    }

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        debug!("linked subsys junk");

        MjpegStream { rx }
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
        // Start the pipeline
        self.pipeline.set_state(State::Playing).unwrap();
        //.expect("Unable to set the pipeline to the `Playing` state.");
    }
    pub fn pause(&self) {
        self.pipeline
            .set_state(State::Paused)
            .expect("Unable to set the pipeline to the `Null` state.");
    }
    #[deprecated]
    pub fn stop(&self) {
        self.pause();
    }
    pub async fn calibrators(&self) -> MutexGuard<HashMap<String, Calibrator>> {
        self.calibrators.lock().await
    }
    pub async fn mjpeg_streams(&self) -> MutexGuard<HashMap<String, MjpegStream>> {
        self.mjpeg_streams.lock().await
    }
}
