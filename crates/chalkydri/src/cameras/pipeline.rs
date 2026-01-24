use std::marker::PhantomData;
use std::sync::Arc;

use gstreamer::{
    Caps, Device, Element, ElementFactory, FlowSuccess, Pipeline, State, Structure, prelude::*,
};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use tokio::sync::watch;

use crate::{config, error::Error, subsystems::{SubsysManager, Subsystem}, Cfg};

use super::mjpeg::MjpegProc;

/// A camera pipeline
///
/// Each camera gets its own GStreamer pipeline.
pub struct CamPipeline {
    dev: Device,
    cam_config: config::Camera,
    pipeline: Pipeline,
    //calibrator: Calibrator,

    input: Element,
    filter: Element,
    jpegdec: Element,
    videoflip: Element,
    tee: Element,

    subsys: SubsysManager,

    pub mjpeg_preproc: PreprocWrap<MjpegProc>,
}
impl CamPipeline {
    /// Create a new camera pipeline from a [Device] and camera config
    pub async fn new(dev: Device, cam_config: config::Camera) -> Self {
        let pipeline = Pipeline::new();

        let input = dev.create_element(Some("camera")).unwrap();
        input.set_state(State::Ready).unwrap();

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

        // MJPEG video must be decoded into raw video before we can use it
        let jpegdec = ElementFactory::make_with_name("jpegdec", None).unwrap();

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

        pipeline
            .add_many([&input, &filter, &jpegdec, &videoflip, &tee])
            .unwrap();

        // If we're getting an MJPEG stream from the cam, it needs to first be decoded
        if is_mjpeg {
            Element::link_many([&input, &filter, &jpegdec, &videoflip, &tee])
                .unwrap();
        } else {
            Element::link_many([&input, &filter, &videoflip, &tee])
                .unwrap();
        }

        let mjpeg_preproc = PreprocWrap::<MjpegProc>::new(&pipeline);
        mjpeg_preproc.setup_sampler(Some(mjpeg_preproc.inner().tx.clone())).unwrap();

        let subsys = SubsysManager::new(&pipeline).await.unwrap();

        Self {
            dev,
            cam_config,
            pipeline,

            input,
            filter,
            jpegdec,
            videoflip,
            tee,

            mjpeg_preproc,
            subsys,
        }
    }

    /// Link subsystem preprocessors
    pub(crate) async fn link_preprocs(&self, cam_config: config::Camera) {
        //if cam_config.subsystems.mjpeg.is_some() {
            self.mjpeg_preproc.link(self.tee.clone());
            //self.subsys.start(self.cam_config.clone(), &self.pipeline, &self.tee).await;
        //}
    }

    /// Unlink subsystem preprocessors
    pub(crate) async fn unlink_preprocs(&self, cam_config: config::Camera) {
        //if cam_config.subsystems.mjpeg.is_some() {
        //self.subsys.stop().await;
        self.mjpeg_preproc.unlink(self.tee.clone());
        //}
    }

    /// Start the pipeline
    pub async fn start(&self) {
        self.pipeline.set_state(State::Playing).unwrap();
        self.subsys.start(self.cam_config.clone(), &self.pipeline, &self.tee).await;
    }

    /// Pause the pipeline
    pub fn pause(&self) {
        self.pipeline.set_state(State::Paused).unwrap();
    }

    /// Update the pipeline
    pub async fn update(&self, cam_config: config::Camera) {
        self.pause();

        if let Some(settings) = &cam_config.settings {
            let capsfilter = self.pipeline.by_name("capsfilter").unwrap();
            let mut old_caps = self
                .pipeline
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
            self.pipeline.foreach_sink_pad(|_elem, pad| {
                pad.mark_reconfigure();
                true
            });

            let camera = self.pipeline.by_name("camera").unwrap();

            //let mut extra_controls = camera.property::<Structure>("extra-controls");
            //extra_controls.set(
            //    "auto_exposure",
            //    if cam_config.auto_exposure { 3 } else { 1 },
            //);
            //if let Some(manual_exposure) = cam_config.manual_exposure {
            //    extra_controls.set("exposure_time_absolute", &manual_exposure);
            //}
            //camera.set_property("extra-controls", extra_controls);

            self.pipeline
                .by_name("videoflip")
                .unwrap()
                .set_property_from_str(
                    "method",
                    &serde_json::to_string(&cam_config.orientation)
                        .unwrap()
                        .trim_matches('"'),
                );

            if let Some(capriltags_valve) = self.pipeline.by_name("capriltags_valve") {
                capriltags_valve.set_property("drop", cam_config.subsystems.capriltags.is_none());
            }
        }

        self.start().await;
    }
}

/// A set of Gstreamer elements used to preprocess the stream for a [Subsystem]
pub trait Preprocessor {
    type Subsys: Subsystem;
    type Frame: Clone + Send + Sync + 'static;

    fn new(pipeline: &Pipeline) -> Self;
    fn link(&self, src: Element, sink: Element);
    fn unlink(&self, src: Element, sink: Element);
    fn sampler(
        appsink: &AppSink,
        tx: watch::Sender<Option<Arc<Self::Frame>>>,
    ) -> Result<Option<()>, Error>;
}

/// A no-op preprocessor for subsystems that don't require any preprocessing
pub struct NoopPreproc<S: Subsystem>(PhantomData<S>);
impl<S: Subsystem> NoopPreproc<S> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<S: Subsystem> Preprocessor for NoopPreproc<S> {
    type Subsys = S;
    type Frame = ();

    fn new(_pipeline: &Pipeline) -> Self {
        Self::new()
    }
    fn link(&self, _src: Element, _dst: Element) {}
    fn unlink(&self, _src: Element, _dst: Element) {}
    fn sampler(
        appsink: &AppSink,
        tx: watch::Sender<Option<Arc<Self::Frame>>>,
    ) -> Result<Option<()>, Error> {
        Ok(None)
    }
}

/// Wrapper around [Preprocessor] implementations that handles the [AppSink] junk
pub struct PreprocWrap<P: Preprocessor> {
    inner: P,
    appsink: Element,
    tx: watch::Sender<Option<Arc<P::Frame>>>,
    rx: watch::Receiver<Option<Arc<P::Frame>>>,
}
impl<P: Preprocessor> PreprocWrap<P> {
    /// Create a new wrapped preprocessor
    pub fn new(pipeline: &Pipeline) -> Self {
        let inner = <P as Preprocessor>::new(pipeline);

        let appsink = ElementFactory::make("appsink")
            .build()
            .unwrap();

        if let Err(err) = pipeline.add(&appsink) {
            error!("failed to add appsink to pipeline: {err:?}");
        }

        let (tx, rx) = watch::channel(None);

        Self { inner, appsink, tx, rx }
    }

    /// Link the preprocessor
    pub fn link(&self, src: Element) {
        let appsink = self.appsink.clone();
        self.inner.link(src, appsink);
    }

    /// Unlink the preprocessor
    pub fn unlink(&self, src: Element) {
        let appsink = self.appsink.clone();
        self.inner.unlink(src, appsink);
    }

    /// Set up the sampler
    pub fn setup_sampler(
        &self,
        tx: Option<watch::Sender<Option<Arc<P::Frame>>>>,
    ) -> Result<Option<()>, Error> {
        let appsink = self.appsink.clone().dynamic_cast::<AppSink>().unwrap();
        appsink.set_drop(true);

        let tx = if let Some(tx) = tx {
            tx.clone()
        } else {
            self.tx.clone()
        };

        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    debug!("got sample");
                    P::sampler(appsink, tx.clone()).unwrap();
                    Ok(FlowSuccess::Ok)
                })
                .build(),
        );

        Ok(None)
    }

    /// Get the inner preprocessor
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Get the preprocessed frame buffer
    pub fn rx(&self) -> watch::Receiver<Option<Arc<P::Frame>>> {
        self.rx.clone()
    }
}
