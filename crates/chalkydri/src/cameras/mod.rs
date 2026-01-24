/***
 * THIS FILE IS CURSED
 * PLES SEND HELP
 */

pub(crate) mod mjpeg;
pub(crate) mod pipeline;
mod providers;
mod publisher;

use gstreamer::{
    Bin, Bus, BusSyncReply, Caps, Device, DeviceProvider, DeviceProviderFactory, Element,
    ElementFactory, FlowError, FlowSuccess, Fraction, Message, MessageView, PadDirection, Pipeline,
    State, Structure, glib::WeakRef, prelude::*,
};

use gstreamer_app::{AppSink, AppSinkCallbacks};
use nt_client::ClientHandle as NTClientHandle;
use pipeline::CamPipeline;
use providers::{CamProvider, ProviderEvent, V4l2Provider};
use publisher::CamPublisher;
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc};
use tokio::{
    sync::{Mutex, MutexGuard, RwLock, mpsc, watch},
    task::JoinHandle,
};
use tracing::Level;

#[cfg(feature = "rerun")]
use crate::Rerun;
use crate::{
    Cfg,
    config::{self, CameraSettings, CfgFraction},
    error::Error,
    subsystems::Subsystem,
};

#[derive(Clone)]
pub struct CameraCtx {
    cfgg: config::Camera,
    tee: WeakRef<Element>,
}

#[derive(Clone)]
pub struct CamManager {
    v4l2_prov: Arc<Mutex<V4l2Provider>>,
    pub pipelines: Arc<RwLock<HashMap<String, CamPipeline>>>,

    restart_tx: mpsc::Sender<()>,
    dev_msg_tx: mpsc::Sender<ProviderEvent>,
    pub new_dev_rx: Arc<Mutex<mpsc::Receiver<config::Camera>>>,
}
impl CamManager {
    pub async fn new(nt: &NTClientHandle, restart_tx: mpsc::Sender<()>) -> (Self, impl Future<Output = ()>) {
        let v4l2_prov = Arc::new(Mutex::new(V4l2Provider::init()));

        let pipelines = Arc::new(RwLock::new(HashMap::new()));

        let (dev_msg_tx, dev_msg_rx) = mpsc::channel(20);
        let (new_dev_tx, new_dev_rx) = mpsc::channel(20);

        let pipelines_ = pipelines.clone();
        let runner = Self::spawn_dev_msg_handler(pipelines_, dev_msg_rx, new_dev_tx);

        (
            Self {
                v4l2_prov,
                pipelines,

                restart_tx,
                dev_msg_tx,
                new_dev_rx: Arc::new(Mutex::new(new_dev_rx)),
            },
            runner,
        )
    }

    pub async fn start_dev_providers(&self) {
        self.v4l2_prov
            .lock()
            .await
            .register_handler(self.dev_msg_tx.clone());
        self.v4l2_prov.lock().await.start();
    }
    pub async fn stop_dev_providers(&self) {
        self.v4l2_prov.lock().await.stop();
        self.v4l2_prov.lock().await.unregister_handler();
    }

    async fn spawn_dev_msg_handler(
        pipelines: Arc<RwLock<HashMap<String, CamPipeline>>>,
        mut rx: mpsc::Receiver<ProviderEvent>,
        tx: mpsc::Sender<config::Camera>,
    ) {
        debug!("starting dev msg handler...");
        'outer: while let Some(event) = rx.recv().await {
            match event {
                ProviderEvent::Connected(id, dev) => {
                    let id = V4l2Provider::get_id(&dev);
                    println!("idfk: {id}");

                    if let Some(cameras) = Cfg.read().await.cameras.clone() {
                        for cam in cameras {
                            if id == cam.id {
                                let pipeline = CamPipeline::new(dev.clone(), cam.clone()).await;
                                debug!("linking preprocs");
                                pipeline.link_preprocs(cam).await;
                                debug!("starting pipeline");
                                pipeline.start().await;
                                let _ = pipelines.write().await.insert(id.clone(), pipeline);
                                println!("existing cam: {id}");
                                continue 'outer;
                            }
                        }
                    }

                    println!("new cam: {id}");
                    tx.send(config::Camera {
                        id,
                        possible_settings: Some(
                            dev.caps()
                                .unwrap()
                                .iter()
                                //.filter(|cap| cap.name().as_str() == "video/x-raw")
                                .filter_map(|cap| {
                                    let width = cap.get::<i32>("width").ok().map(|v| v as u32);
                                    let height = cap.get::<i32>("height").ok().map(|v| v as u32);
                                    let frame_rate =
                                        cap.get::<Fraction>("framerate").ok().map(|v| {
                                            CfgFraction {
                                                num: v.numer() as u32,
                                                den: v.denom() as u32,
                                            }
                                        });
                                    if width.is_none() || height.is_none() {
                                        //panic!("Either width or height doesn't exist. Need to look into that...");
                                        None
                                    } else {
                                        Some(CameraSettings {
                                            width: width.unwrap(),
                                            height: height.unwrap(),
                                            frame_rate,
                                            format: Some(
                                                cap.get::<String>("format").unwrap_or_default(),
                                            ),
                                        })
                                    }
                                })
                                .collect(),
                        ),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                }
                ProviderEvent::Disconnected(id, dev) => {
                    let mut pipelines = pipelines.write().await;
                    if pipelines.contains_key(&id) {
                        pipelines.remove(&id);
                    }
                }
            }
        }

        panic!("dev msg handler died");
    }

    pub async fn refresh_devices(&self) {
        let mut cfgg = Cfg.write().await;

        let mut cameras = cfgg.cameras.clone();

        if let Some(ref mut cams) = cameras {
            for mut cam in cams {
                cam.online = self.pipelines.read().await.contains_key(&cam.id);
            }
        }

        (*cfgg).cameras = cameras;
    }

    pub async fn update_pipeline(&self, cam_id: String) {
        let cfgg = Cfg.read().await.clone();
        let cam_config = cfgg
            .cameras
            .unwrap()
            .iter()
            .filter(|cam| cam.id == cam_id)
            .next()
            .unwrap()
            .clone();
        self.pipelines
            .read()
            .await
            .get(&cam_id)
            .unwrap()
            .update(cam_config)
            .await;
    }

    // /// Add [Calibrator] to pipeline
    // pub(crate) fn add_calib(
    //     pipeline: &Pipeline,
    //     cam: &Element,
    //     cam_config: config::Camera,
    // ) -> Calibrator {
    //     let span = span!(Level::INFO, "calib");
    //     let _enter = span.enter();

    //     let bin = Bin::builder().name("calib").build();

    //     let valve = ElementFactory::make("valve")
    //         .property("drop", false)
    //         .build()
    //         .unwrap();
    //     let queue = ElementFactory::make("queue").build().unwrap();
    //     let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
    //     let filter = ElementFactory::make("capsfilter")
    //         .property(
    //             "caps",
    //             &Caps::builder("video/x-raw")
    //                 .field("width", &1280)
    //                 .field("height", &720)
    //                 .field("format", "RGB")
    //                 .build(),
    //         )
    //         .build()
    //         .unwrap();
    //     let appsink = ElementFactory::make("appsink")
    //         .name("calib_appsink")
    //         .build()
    //         .unwrap();

    //     bin.add_many([&valve, &queue, &videoconvertscale, &filter, &appsink])
    //         .unwrap();
    //     Element::link_many([&cam, &valve, &queue, &videoconvertscale, &filter, &appsink]).unwrap();

    //     let appsink = appsink.dynamic_cast::<AppSink>().unwrap();
    //     appsink.set_drop(true);

    //     let (tx, rx) = watch::channel(None);

    //     debug!("setting appsink callbacks...");
    //     appsink.set_callbacks(
    //         AppSinkCallbacks::builder()
    //             .new_sample(move |appsink| {
    //                 let sample = appsink.pull_sample().unwrap();
    //                 let buf = sample.buffer().unwrap();
    //                 while let Err(err) = tx.send(Some(buf.to_owned())) {
    //                     error!("error sending frame: {err:?}");
    //                 }

    //                 Ok(FlowSuccess::Ok)
    //             })
    //             .build(),
    //     );

    //     debug!("linked subsys junk");

    //     Calibrator::new(valve.downgrade(), rx)
    // }

    // pub async fn calibrators(&self) -> MutexGuard<HashMap<String, Calibrator>> {
    //     self.calibrators.lock().await
    // }

    // /// Run a calibration step
    // pub async fn calib_step(&self, name: String) -> usize {
    //     self.calibrators().await.get_mut(&name).unwrap().step()
    // }
}

/*
/// The camera manager
///
/// This manages all of the GStreamer pipelines.
/// It also handles device events.
#[derive(Clone)]
pub struct CameraManager {
    dev_prov: DeviceProvider,
    pipelines: Arc<RwLock<HashMap<String, Pipeline>>>,
    calibrators: Arc<Mutex<HashMap<String, Calibrator>>>,
    mjpeg_streams: Arc<Mutex<HashMap<String, MjpegStream>>>,
    restart_tx: mpsc::Sender<()>,
    subsys_man: SubsysManager,
    publisher: Arc<Mutex<CamPublisher>>,
}
impl CameraManager {
    /// Initialize a camera manager
    ///
    /// **You MUST call [gstreamer::init] first.**
    pub async fn new(nt: NtConn, restart_tx: mpsc::Sender<()>) -> Self {
        // Make sure gstreamer is initialized
        gstreamer::assert_initialized();

        // Get a copy of the global configuration
        let config = {
            let cfgg = Cfg.read().await;
            let ret = (*cfgg).clone();
            drop(cfgg);
            ret
        };

        let pipelines = Arc::new(RwLock::new(HashMap::new()));
        let calibrators: Arc<Mutex<HashMap<String, Calibrator>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let mjpeg_streams: Arc<Mutex<HashMap<String, MjpegStream>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let publisher = Arc::new(Mutex::new(CamPublisher::new()));

        let subsys_man = SubsysManager::new().await.unwrap();

        // Create a device monitor to watch for new devices
        let dev_prov = DeviceProviderFactory::find("v4l2deviceprovider")
            .unwrap()
            .load()
            .unwrap()
            .get()
            .unwrap();

        let bus = dev_prov.bus();

        let calibrators_ = calibrators.clone();
        let mjpeg_streams_ = mjpeg_streams.clone();
        let pipelines_ = pipelines.clone();
        let publisher_ = publisher.clone();
        let manager = subsys_man.clone();
        let rt_handle = tokio::runtime::Handle::current();
        bus.set_sync_handler(move |_, msg| {
            let calibrators = &calibrators_;
            let mjpeg_streams = &mjpeg_streams_;
            let pipelines = &pipelines_;
            let publisher = &publisher_;
            match msg.view() {
                MessageView::DeviceAdded(msg) => {
                    let dev = msg.device();
                    debug!("got a new device");

                    // Create a new pipeline
                    let pipeline = Pipeline::new();

                    if let Some(cam_configs) = &config.cameras {
                        let id = Self::get_id(&dev);

                        if let Some(cam_config) =
                            cam_configs.clone().iter().filter(|cam| cam.id == id).next()
                        {
                            let span = span!(Level::INFO, "camera", id = cam_config.id);
                            let _enter = span.enter();

                            debug!("found a config");

                            // Create the camera source
                            let cam = dev.create_element(Some("camera")).unwrap();

                            let mut extra_controls = Structure::new_empty("extra-controls");
                            extra_controls.set(
                                "auto_exposure",
                                if cam_config.auto_exposure { 3 } else { 1 },
                            );
                            if let Some(manual_exposure) = cam_config.manual_exposure {
                                extra_controls.set("exposure_time_absolute", &manual_exposure);
                            }
                            cam.set_property("extra-controls", extra_controls);

                            // The camera preprocessing part:
                            //   [src]> capsfilter -> queue -> tee -> ...

                            // Create the elements
                            let settings = cam_config.settings.clone().unwrap_or_default();

                            let is_mjpeg = settings.format == Some(String::new());

                            let filter = ElementFactory::make("capsfilter")
                                .name("capsfilter")
                                .property(
                                    "caps",
                                    &Caps::builder(if is_mjpeg {
                                        "image/jpeg"
                                    } else {
                                        "video/x-raw"
                                    })
                                    .field("width", settings.width as i32)
                                    .field("height", settings.height as i32)
                                    //.field(
                                    //    "framerate",
                                    //    &Fraction::new(
                                    //        settings.frame_rate.num as i32,
                                    //        settings.frame_rate.den as i32,
                                    //    ),
                                    //)
                                    .build(),
                                )
                                .build()
                                .unwrap();

                            // This element rotates/flips the video to deal with weird
                            // mounting configurations
                            let videoflip = ElementFactory::make("videoflip")
                                .name("videoflip")
                                .property_from_str(
                                    "method",
                                    &serde_json::to_string(&cam_config.orientation)
                                        .unwrap()
                                        .trim_matches('"'),
                                )
                                .build()
                                .unwrap();

                            // This element splits the stream off into multiple branches of the
                            // pipeline:
                            //  - MJPEG stream
                            //  - Calibration
                            //  - Subsystems
                            let tee = ElementFactory::make("tee").build().unwrap();

                            if is_mjpeg {
                                let jpegdec =
                                    ElementFactory::make_with_name("jpegdec", Some("jpegdec"))
                                        .unwrap();
                                // Add them to the pipeline
                                pipeline
                                    .add_many([&cam, &filter, &jpegdec, &videoflip, &tee])
                                    .unwrap();

                                // Link them
                                Element::link_many([&cam, &filter, &jpegdec, &videoflip, &tee])
                                    .unwrap();
                            } else {
                                // Add them to the pipeline
                                pipeline
                                    .add_many([&cam, &filter, &videoflip, &tee])
                                    .unwrap();

                                // Link them
                                Element::link_many([&cam, &filter, &videoflip, &tee]).unwrap();
                            }

                            debug!("initializing calibrator");
                            let calibrator = Self::add_calib(&pipeline, &tee, cam_config.clone());

                            {
                                debug!("adding calibrator");
                                let mut calibrators =
                                    tokio::task::block_in_place(|| calibrators.blocking_lock());
                                (*calibrators).insert(id.clone(), calibrator);
                                drop(calibrators);
                                debug!("dropped lock");
                            }

                            debug!("initializing mjpeg stream");
                            let mjpeg_stream = Self::add_mjpeg(&pipeline, &tee, cam_config.clone());

                            {
                                debug!("adding mjpeg stream");
                                let mut mjpeg_streams =
                                    tokio::task::block_in_place(|| mjpeg_streams.blocking_lock());
                                (*mjpeg_streams).insert(id.clone(), mjpeg_stream);
                                drop(mjpeg_streams);
                                debug!("dropped lock");
                            }

                            rt_handle.block_on(async {
                                manager.spawn(cam_config.clone(), &pipeline, &tee).await;
                            });

                            tokio::task::block_in_place(|| pipelines.blocking_write())
                                .insert(id, pipeline);

                            publisher_.lock().await.publish(cam_config).await;
                        }
                    }
                }
                _ => {}
            }

            BusSyncReply::Pass
        });

        // Start the device provider
        dev_prov.start().unwrap();

        for (cam_name, pipeline) in pipelines.read().await.clone() {
            debug!("starting pipeline for {cam_name}");

            // Start the pipeline
            pipeline.set_state(State::Playing).unwrap();

            // Get the pipeline's bus
            let bus = pipeline.bus().unwrap();
            // Hook up event handler for the pipeline
            bus.set_sync_handler(|_, _| BusSyncReply::Pass);
        }

        Self {
            dev_prov,
            pipelines,
            calibrators,
            mjpeg_streams,
            restart_tx,
            subsys_man,
            publisher,
        }
    }

    /// Trigger a restart of Chalkydri
    pub async fn restart(&self) {
        self.restart_tx.send(()).await.unwrap();
    }

    /// Get unique identifier for the given device
    fn get_id(dev: &Device) -> String {
        dev.property::<Structure>("properties")
            .get::<String>("device.serial")
            .unwrap()
    }

    /// Update the given pipeline
    pub async fn update_pipeline(&self, dev_id: String) {
        self.pause(dev_id.clone()).await;
        {
            let mut pipelines = self.pipelines.write().await;
            if let Some(pipeline) = pipelines.get_mut(&dev_id) {
                // Get a copy of the global configuration
                let config = {
                    let cfgg = Cfg.read().await;
                    let ret = (*cfgg).clone();
                    drop(cfgg);
                    ret
                };

                if let Some(cam_configs) = &config.cameras {
                    if let Some(cam_config) = cam_configs
                        .clone()
                        .iter()
                        .filter(|cam| cam.id == dev_id)
                        .next()
                    {
                        if let Some(settings) = &cam_config.settings {
                            let capsfilter = pipeline.by_name("capsfilter").unwrap();
                            let mut old_caps = pipeline
                                .by_name("capsfilter")
                                .unwrap()
                                .property::<Caps>("caps")
                                .to_owned();
                            let caps = old_caps.make_mut();
                            caps.set_value("width", (&(settings.width as i32)).into());
                            caps.set_value("height", (&(settings.height as i32)).into());
                            //caps.set_value(
                            //            "framerate",
                            //            (&Fraction::new(
                            //                settings.frame_rate.num as i32,
                            //                settings.frame_rate.den as i32,
                            //            )).into(),
                            //);
                            capsfilter.set_property("caps", caps.to_owned());

                            // Reconfigure [Caps]
                            pipeline.foreach_sink_pad(|_elem, pad| {
                                pad.mark_reconfigure();
                                true
                            });

                            let camera = pipeline.by_name("camera").unwrap();

                            let mut extra_controls = camera.property::<Structure>("extra-controls");
                            extra_controls.set(
                                "auto_exposure",
                                if cam_config.auto_exposure { 3 } else { 1 },
                            );
                            if let Some(manual_exposure) = cam_config.manual_exposure {
                                extra_controls.set("exposure_time_absolute", &manual_exposure);
                            }
                            camera.set_property("extra-controls", extra_controls);

                            pipeline
                                .by_name("videoflip")
                                .unwrap()
                                .set_property_from_str(
                                    "method",
                                    &serde_json::to_string(&cam_config.orientation)
                                        .unwrap()
                                        .trim_matches('"'),
                                );

                            if let Some(capriltags_valve) = pipeline.by_name("capriltags_valve") {
                                capriltags_valve.set_property(
                                    "drop",
                                    cam_config.subsystems.capriltags.is_none(),
                                );
                            }
                        }
                    }
                }
            }
        }
        self.start(dev_id).await;
    }

    /// List connected cameras
    pub fn devices(&self) -> Vec<config::Camera> {
        let mut devices = Vec::new();

        for dev in self.dev_prov.devices().iter() {
            let id = Self::get_id(&dev);
            devices.push(config::Camera {
                id,
                possible_settings: Some(
                    dev.caps()
                        .unwrap()
                        .iter()
                        //.filter(|cap| cap.name().as_str() == "video/x-raw")
                        .filter_map(|cap| {
                            let width = cap.get::<i32>("width").ok().map(|v| v as u32);
                            let height = cap.get::<i32>("height").ok().map(|v| v as u32);
                            let frame_rate = cap
                                .get::<Fraction>("framerate").ok().map(|v| CfgFraction {
                                    num: v.numer() as u32,
                                    den: v.denom() as u32,
                                });
                            if width.is_none() || height.is_none() {
                                error!("Either width or height doesn't exist. Need to look into that...");
                                None
                            } else {
                                Some(CameraSettings {
                                    width: width.unwrap(),
                                    height: height.unwrap(),
                                    frame_rate,
                                    format: Some(cap.get::<String>("format").unwrap_or_default()),
                                })
                            }
                        })
                        .collect(),
                ),
                ..Default::default()
            });
        }

        devices
    }

    /// Run a calibration step
    pub async fn calib_step(&self, name: String) -> usize {
        self.calibrators().await.get_mut(&name).unwrap().step()
    }

    /// Get an [`MJPEG stream`](MjpegStream)
    pub async fn mjpeg_stream(&self, name: String) -> MjpegStream {
        self.mjpeg_streams().await.get(&name).unwrap().clone()
    }

    /// Add [`subsystem`](Subsystem) to pipeline
    pub(crate) fn add_subsys<S: Subsystem>(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
        enabled: bool,
    ) -> watch::Receiver<Option<Vec<u8>>> {
        let span = span!(Level::INFO, "subsys", subsystem = S::NAME);
        let _enter = span.enter();

        debug!("initializing preproc pipeline chunk subsystem...");
        let (input, output) = S::preproc(cam_config.clone(), pipeline).unwrap();

        let valve = ElementFactory::make("valve")
            .name(&format!("{}_valve", S::NAME))
            .property("drop", !enabled)
            .build()
            .unwrap();
        let videorate = ElementFactory::make("videorate")
            .property("max-rate", 40)
            .property("drop-only", true)
            .build()
            .unwrap();
        let appsink = ElementFactory::make("appsink")
            .name(&format!("{}_appsink", S::NAME))
            .build()
            .unwrap();
        pipeline.add_many([&valve, &videorate, &appsink]).unwrap();

        debug!("linking preproc pipeline chunk...");
        Element::link_many([&cam, &valve, &videorate, &input]).unwrap();
        output.link(&appsink).unwrap();

        let appsink = appsink.dynamic_cast::<AppSink>().unwrap();
        appsink.set_drop(true);

        let (tx, rx) = watch::channel(None);

        let appsink_ = appsink.clone();

        debug!("setting appsink callbacks...");
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |_| match appsink_.pull_sample() {
                    Ok(sample) => {
                        let buf = sample.buffer().unwrap();
                        let buf = buf
                            .to_owned()
                            .into_mapped_buffer_readable()
                            .unwrap()
                            .to_vec();
                        if let Err(err) = tx.send(Some(buf)) {
                            error!("failed to send frame to subsys appsink: {err:?}");
                        }

                        Ok(FlowSuccess::Ok)
                    }
                    Err(err) => {
                        error!("failed to pull sample: {err:?}");
                        Err(FlowError::Error)
                    }
                })
                .build(),
        );
        appsink.set_async(false);

        debug!("linked subsys junk");

        //let cam_config = cam_config.clone();
        //std::thread::spawn(move || {
        //    debug!("capriltags worker thread started");
        //    futures_executor::block_on(async move {
        //        debug!("initializing subsystem...");
        //        let mut subsys = S::init(cam_config).await.unwrap();

        //        debug!("starting subsystem...");
        //        subsys.process(Nt.clone(), rx).await.unwrap();
        //    });
        //});

        rx
    }

    /// Add [Calibrator] to pipeline
    pub(crate) fn add_calib(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
    ) -> Calibrator {
        let span = span!(Level::INFO, "calib");
        let _enter = span.enter();

        let bin = Bin::builder().name("calib").build();

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
        let appsink = ElementFactory::make("appsink")
            .name("calib_appsink")
            .build()
            .unwrap();

        bin.add_many([&valve, &queue, &videoconvertscale, &filter, &appsink])
            .unwrap();
        Element::link_many([&cam, &valve, &queue, &videoconvertscale, &filter, &appsink]).unwrap();

        let appsink = appsink.dynamic_cast::<AppSink>().unwrap();
        appsink.set_drop(true);

        let (tx, rx) = watch::channel(None);

        debug!("setting appsink callbacks...");
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

    /// Add [MjpegStream] to pipeline
    pub(crate) fn add_mjpeg(
        pipeline: &Pipeline,
        cam: &Element,
        cam_config: config::Camera,
    ) -> MjpegStream {
        let span = span!(Level::INFO, "mjpeg");
        let _enter = span.enter();

        let valve = ElementFactory::make("valve")
            .property("drop", false)
            .build()
            .unwrap();
        let videorate = ElementFactory::make("videorate")
            .property("max-rate", 20)
            .property("drop-only", true)
            .build()
            .unwrap();
        //let queue = ElementFactory::make("queue").build().unwrap();
        let videoconvertscale = ElementFactory::make("videoconvertscale")
            .property_from_str("method", "nearest-neighbour")
            .build()
            .unwrap();
        let filter = ElementFactory::make("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
                    .field("width", &640)
                    .field("height", &480)
                    .field("format", "RGB")
                    .build(),
            )
            .build()
            .unwrap();
        //let jpegenc = ElementFactory::make("jpegenc")
        //    .property("quality", &85)
        //    .build()
        //    .unwrap();
        let appsink = ElementFactory::make("appsink")
            .name("mjpeg_appsink")
            .build()
            .unwrap();

        pipeline
            .add_many([
                &valve,
                &videorate,
                &videoconvertscale,
                &filter,
                //&jpegenc,
                &appsink,
            ])
            .unwrap();
        Element::link_many([
            &cam,
            &valve,
            &videorate,
            &videoconvertscale,
            &filter,
            //&jpegenc,
            &appsink,
        ])
        .unwrap();

        let appsink = appsink.dynamic_cast::<AppSink>().unwrap();
        appsink.set_drop(true);

        let (tx, rx) = watch::channel(None);

        debug!("setting appsink callbacks...");
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink
                        .pull_sample()
                        .map_err(|_| Error::FailedToPullSample)
                        .unwrap();
                    match sample.buffer() {
                        Some(buf) => {
                            let jpeg = turbojpeg::compress(
                                turbojpeg::Image {
                                    width: 640,
                                    height: 480,
                                    pitch: 640 * 3,
                                    format: turbojpeg::PixelFormat::RGB,
                                    pixels: buf
                                        .to_owned()
                                        .into_mapped_buffer_readable()
                                        .unwrap()
                                        .to_vec()
                                        .as_slice(),
                                },
                                50,
                                turbojpeg::Subsamp::None,
                            )
                            .unwrap();
                            while let Err(err) = tx.send(Some(jpeg.to_vec())) {
                                error!("error sending frame: {err:?}");
                            }
                        }
                        None => {
                            error!("failed to get buffer");
                        }
                    }

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        debug!("linked subsys junk");

        MjpegStream { rx }
    }

    pub async fn run(&self, name: String) -> Result<(), Box<dyn std::error::Error>> {
        // Define the event loop or something?
        self.pipelines
            .read()
            .await
            .get(&name)
            .unwrap()
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

    /// Start the given camera's pipeline
    pub async fn start(&self, name: String) {
        // Start the pipeline
        if let Some(pipeline) = self.pipelines.read().await.get(&name) {
            pipeline.set_state(State::Playing).unwrap();
        }
        //.expect("Unable to set the pipeline to the `Playing` state.");
    }

    /// Pause the given camera's pipeline
    pub async fn pause(&self, name: String) {
        if let Some(pipeline) = self.pipelines.read().await.get(&name) {
            pipeline
                .set_state(State::Paused)
                .expect("Unable to set the pipeline to the `Null` state.");
        }
    }

    pub async fn calibrators(&self) -> MutexGuard<HashMap<String, Calibrator>> {
        self.calibrators.lock().await
    }
    pub async fn mjpeg_streams(&self) -> MutexGuard<HashMap<String, MjpegStream>> {
        self.mjpeg_streams.lock().await
    }
}
*/
