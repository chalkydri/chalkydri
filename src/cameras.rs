/***
 * THIS FILE IS CURSED
 * PLES SEND HELP
 */

use actix_web::web::Bytes;
use futures_core::Stream;
use gstreamer::{
    Buffer, BusSyncReply, Caps, DeviceProvider, DeviceProviderFactory, Element, ElementFactory,
    FlowSuccess, Fraction, MessageView, Pipeline, State, Structure, glib::WeakRef, prelude::*,
};

use gstreamer_app::{AppSink, AppSinkCallbacks};
use minint::{NtConn, NtTopic};
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc, task::Poll};
use tokio::sync::{Mutex, MutexGuard, RwLock, watch};

#[cfg(feature = "rerun")]
use crate::Rerun;
use crate::{
    Cfg,
    calibration::Calibrator,
    config::{self, CameraSettings, CfgFraction},
    error::Error,
    subsys::capriltags::CApriltagsDetector,
    subsystem::Subsystem,
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
    type Item = Result<Bytes, Error>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            match self.rx.has_changed() {
                Ok(true) => {
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
                        bytes.extend_from_slice(
                            frame
                                .map_readable()
                                .map_err(|_| Error::FailedToMapBuffer)?
                                .as_slice(),
                        );
                    }

                    return Poll::Ready(Some(Ok(bytes.into())));
                }
                Ok(false) => {}
                Err(err) => {
                    error!("error getting frame: {err:?}");

                    return Poll::Ready(None);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct CameraManager {
    //dev_mon: DeviceMonitor,
    dev_prov: DeviceProvider,
    pipelines: Arc<RwLock<HashMap<String, Pipeline>>>,
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
        let dev_prov = DeviceProviderFactory::find("v4l2deviceprovider")
            .unwrap()
            .load()
            .unwrap()
            .get()
            .unwrap();

        // Create the pipeline
        let pipelines = Arc::new(RwLock::new(HashMap::new()));

        //let bus = dev_mon.bus();
        let bus = dev_prov.bus();

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
            let pipeline = Pipeline::new();

            match msg.view() {
                MessageView::DeviceAdded(msg) => {
                    let dev = msg.device();
                    debug!("got a new device");

                    if let Some(cam_configs) = &config.cameras {
                        let id = dev
                            .property::<Structure>("properties")
                            .get::<String>("device.serial")
                            .unwrap();
                        if let Some(cam_config) =
                            cam_configs.clone().iter().filter(|cam| cam.id == id).next()
                        {
                            debug!("found a config");
                            dbg!(
                                dev.list_properties()
                                    .iter()
                                    .map(|prop| prop.name().to_string())
                                    .collect::<Vec<_>>()
                            );
                            dbg!(
                                dev.property::<Structure>("properties")
                                    .iter()
                                    .map(|(k, v)| (k.to_string(), v.to_value()))
                                    .collect::<Vec<_>>()
                            );

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
                            let filter = ElementFactory::make("capsfilter")
                                .name("capsfilter")
                                .property(
                                    "caps",
                                    &Caps::builder("video/x-raw")
                                        .field("width", &1280)
                                        .field("height", &720)
                                        .build(),
                                )
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

                            if cam_config.calib.is_some() {
                                Self::add_subsys::<CApriltagsDetector>(
                                    &pipeline,
                                    &tee,
                                    cam_config.clone(),
                                    nt.clone(),
                                    cam_config.subsystems.capriltags.enabled,
                                );
                            }

                            tokio::task::block_in_place(|| pipelines.blocking_write())
                                .insert(id, pipeline);

                            futures_executor::block_on(async {
                                let mut streams = ManuallyDrop::new(
                                    nt.publish(format!(
                                        "/CameraPublisher/{}/streams",
                                        cam_config.name
                                    ))
                                    .await
                                    .unwrap(),
                                );
                                streams
                                    .set(vec![format!(
                                        "mjpeg:http://localhost:6942/stream/{}",
                                        cam_config.id
                                    )])
                                    .await
                                    .unwrap();
                            });
                        }
                    }
                }
                _ => {}
            }

            BusSyncReply::Pass
        });

        // Start the device monitor
        dev_prov.start().unwrap();

        for (cam_name, pipeline) in pipelines.read().await.clone() {
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
        }
    }

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
                            //pipeline.by_name("capsfilter").unwrap().set_property(
                            //    "caps",
                            //    &Caps::builder("video/x-raw")
                            //        .field("width", &settings.width)
                            //        .field("height", &settings.height)
                            //        //.field(
                            //        //    "framerate",
                            //        //    &Fraction::new(
                            //        //        settings.frame_rate.num as i32,
                            //        //        settings.frame_rate.den as i32,
                            //        //    ),
                            //        //)
                            //        .build(),
                            //);

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
                                .by_name("capriltags_valve")
                                .unwrap()
                                .set_property("drop", !cam_config.subsystems.capriltags.enabled);
                        }
                    }
                }
            }
        }
        self.start(dev_id).await;
    }

    pub async fn destroy_pipeline(&self, dev_id: String) {
        let mut pipelines = self.pipelines.write().await;
        unsafe {
            pipelines
                .get_mut(&dev_id)
                .unwrap()
                .set_state(State::Null)
                .unwrap();
            pipelines.get_mut(&dev_id).unwrap().run_dispose();
        }
        pipelines.remove(&dev_id);
        self.pipelines.write().await.remove(&dev_id);
    }

    /// List connected cameras
    pub fn devices(&self) -> Vec<config::Camera> {
        let mut devices = Vec::new();

        for dev in self.dev_prov.devices().iter() {
            let id = dev
                .property::<Structure>("properties")
                .get::<String>("device.serial")
                .unwrap();
            devices.push(config::Camera {
                id,
                name: String::new(),
                settings: None,
                auto_exposure: true,
                manual_exposure: None,
                possible_settings: Some(
                    dev.caps()
                        .unwrap()
                        .iter()
                        .map(|cap| {
                            let frame_rate = cap
                                .get::<Fraction>("framerate")
                                .unwrap_or_else(|_| Fraction::new(30, 1));
                            CameraSettings {
                                width: cap.get::<i32>("width").unwrap_or(1280) as u32,
                                height: cap.get::<i32>("height").unwrap_or(720) as u32,
                                frame_rate: CfgFraction {
                                    num: frame_rate.numer() as u32,
                                    den: frame_rate.denom() as u32,
                                },
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
        enabled: bool,
    ) {
        let target = format!("chalkydri::subsys::{}", S::NAME);

        debug!(target: &target, "initializing preproc pipeline chunk subsystem...");
        let (input, output) = S::preproc(cam_config.clone(), pipeline).unwrap();

        let valve = ElementFactory::make("valve")
            .name(&format!("{}_valve", S::NAME))
            .property("drop", !enabled)
            .build()
            .unwrap();
        let queue = ElementFactory::make("queue").build().unwrap();
        let appsink = ElementFactory::make("appsink").build().unwrap();
        pipeline.add_many([&valve, &queue, &appsink]).unwrap();

        debug!(target: &target, "linking preproc pipeline chunk...");
        Element::link_many([&cam, &valve, &queue, &input]).unwrap();
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
                    if let Err(err) = tx.send(Some(buf.to_owned())) {
                        error!("failed to send frame to subsys appsink: {err:?}");
                    }

                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );
        appsink.set_async(false);

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
                    .field("width", &640)
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
                    let sample = appsink
                        .pull_sample()
                        .map_err(|_| Error::FailedToPullSample)
                        .unwrap();
                    match sample.buffer() {
                        Some(buf) => {
                            while let Err(err) = tx.send(Some(buf.to_owned())) {
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

    pub async fn start(&self, name: String) {
        // Start the pipeline
        if let Some(pipeline) = self.pipelines.read().await.get(&name) {
            pipeline.set_state(State::Playing).unwrap();
        }
        //.expect("Unable to set the pipeline to the `Playing` state.");
    }
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
