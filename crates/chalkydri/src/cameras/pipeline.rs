use std::sync::mpsc::{self, Sender};
use std::time::Duration;
use std::{marker::PhantomData, ops::ControlFlow};
use std::sync::Arc;

use cu_gstreamer::CuGstBuffer;
use cu29::prelude::*;
use gstreamer::{
    Caps, DebugGraphDetails, Device, Element, ElementFactory, FlowSuccess, Pipeline, Sample, State, Structure, prelude::*
};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use tokio::sync::watch;

use crate::cameras::providers::{CamProvider, CamProviderBundle, CamProviderBundleId, V4l2Provider};
use crate::{cameras::preproc::PreprocWrap, subsystems::SubsysManager};
use chalkydri_core::prelude::*;

use super::mjpeg::MjpegProc;

/// A camera pipeline
///
/// Each camera gets its own GStreamer pipeline.
pub struct CamPipeline {
    dev: Device,
    cam_config: crate::config::Camera,
    pipeline: Pipeline,
    //calibrator: Calibrator,
    input: Element,
    filter: Element,
    jpegdec: Element,
    videoflip: Element,
    appsink: AppSink,
    sample_queue: Arc<mpsc::Receiver<Sample>>,

    //pub mjpeg_preproc: PreprocWrap<MjpegProc>,
}
impl CamPipeline {
    /// Create a new camera pipeline from a [Device] and camera config
    pub fn new(dev: Device, cam_config: crate::config::Camera) -> Self {
        let pipeline = Pipeline::new();

        let input = dev.create_element(Some("camera")).unwrap();
        //input.set_state(State::Ready).unwrap();

        let settings = cam_config.settings.clone().unwrap_or_default();
        let is_mjpeg = settings.format == Some(String::new());

        let prefilter = ElementFactory::make("capsfilter")
            .name("precapsfilter")
            //.property(
            //    "caps",
            //    &Caps::builder(if is_mjpeg {
            //        "image/jpeg"
            //    } else {
            //        "video/x-raw"
            //    })
            //    .field("width", settings.width as i32)
            //    .field("height", settings.height as i32)
            //    //.field(
            //    //    "framerate",
            //    //    &Fraction::new(
            //    //        settings.frame_rate.num as i32,
            //    //        settings.frame_rate.den as i32,
            //    //    ),
            //    //)
            //    .build(),
            //)
            .property(
                "caps",
                &Caps::builder(
                    "video/x-raw"
                )
                //.field("width", settings.width as i32)
                .field("width", 1280)
                //.field("height", settings.height as i32)
                .field("height", 720)
                //.field("format", "DMA_DRM")
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

        let videoconvert = ElementFactory::make("videoconvert").name("videoconvert").build().unwrap();

        let filter = ElementFactory::make("capsfilter")
            .name("capsfilter")
            //.property(
            //    "caps",
            //    &Caps::builder(if is_mjpeg {
            //        "image/jpeg"
            //    } else {
            //        "video/x-raw"
            //    })
            //    .field("width", settings.width as i32)
            //    .field("height", settings.height as i32)
            //    //.field(
            //    //    "framerate",
            //    //    &Fraction::new(
            //    //        settings.frame_rate.num as i32,
            //    //        settings.frame_rate.den as i32,
            //    //    ),
            //    //)
            //    .build(),
            //)
            .property(
                "caps",
                &Caps::builder(
                    "video/x-raw"
                )
                //.field("width", settings.width as i32)
                .field("width", 1280)
                //.field("height", settings.height as i32)
                .field("height", 720)
                .field("format", "GRAY8")
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

        let appsink = ElementFactory::make("appsink").build().unwrap();
        //let appsink = AppSink::builder().async_(true).build().unwrap();

        pipeline
            .add_many([&input, &prefilter, &videoconvert, &filter, &jpegdec, &videoflip, &appsink])
            .unwrap();

        // If we're getting an MJPEG stream from the cam, it needs to first be decoded
        // if is_mjpeg {
        //     Element::link_many([&input, &jpegdec, &videoflip, &appsink]).unwrap();
        // } else {
            Element::link_many([&input, &prefilter, &videoconvert, &filter, &videoflip, &appsink]).unwrap();
        //}

        //let mjpeg_preproc = PreprocWrap::<MjpegProc>::new(&pipeline);
        //mjpeg_preproc
        //    .setup_sampler(Some(mjpeg_preproc.inner().tx.clone()))
        //    .unwrap();


        let (tx, rx) = mpsc::sync_channel(64);
        let sample_queue = Arc::new(rx);
        let appsink = appsink.clone().dynamic_cast::<AppSink>().unwrap();
        appsink.set_callbacks(AppSinkCallbacks::builder().new_sample(move |appsink: &AppSink| {
            println!("got sampleeeeeee");
            let sample = appsink.pull_sample().unwrap();
            tx.send(sample.clone()).unwrap();
            Ok(FlowSuccess::Ok)
        }).build());
        appsink.set_sync(true);
        appsink.set_enable_last_sample(false);

        Self {
            dev,
            cam_config,
            pipeline,

            input,
            filter,
            jpegdec,
            videoflip,
            appsink,
            sample_queue,

            //mjpeg_preproc,
        }
    }

    /// Start the pipeline
    #[instrument(skip(self), fields(cam = self.cam_config.id))]
    pub fn start_pipeline(&self) {
        trace!("starting pipeline");
        self.pipeline.set_state(State::Playing).unwrap();
    }

    /// Pause the pipeline
    #[instrument(skip(self), fields(cam = self.cam_config.id))]
    pub fn pause(&self) {
        trace!("pausing pipeline");
        self.pipeline.set_state(State::Paused).unwrap();
    }

    /// Update the pipeline
    #[instrument(skip(self), fields(cam = self.cam_config.id))]
    pub async fn update(&self, cam_config: crate::config::Camera) {
        trace!("pausing pipeline");
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

            trace!("marking pads for reconfiguration");
            self.pipeline.foreach_sink_pad(|_elem, pad| {
                pad.mark_reconfigure();
                ControlFlow::Continue(())
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

        trace!("strating");
        self.start_pipeline();
    }
}

pub struct Resources { pub v4l2: Owned<V4l2Provider> }
impl<'r> ResourceBindings<'r> for Resources {
    type Binding = CamProviderBundleId;
    fn from_bindings(mgr: &'r mut ResourceManager, map: Option<&ResourceBindingMap<Self::Binding>>) -> CuResult<Self> {
        let key = map.expect("v4l2 binding").get(Self::Binding::V4L2).expect("v4l2").typed();
        Ok(Self { v4l2: mgr.take(key)? })
    }
}

impl Freezable for CamPipeline {}
impl CuSrcTask for CamPipeline {
    type Resources<'r> = Resources;
    type Output<'m> = output_msg!(CuGstBuffer);

    fn new(_config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized
    {
        let cam_provider = resources.v4l2.0;

        let rc = _config.unwrap();
        let cfgg = crate::config::Camera {
            id: rc.get("id").unwrap(),
            name: rc.get("name").unwrap(),
            //calib: rc.get("calib").unwrap(),
            //settings: rc.get("settings"),
            //possible_settings: rc.get("possible_settings"),
            auto_exposure: rc.get("auto_exposure").unwrap_or(true),
            manual_exposure: rc.get("manual_exposure"),
            ..Default::default()
        };
        cam_provider.start();
        std::thread::sleep(Duration::from_secs(2));
        let dev = cam_provider.get_by_id(cfgg.id.clone()).unwrap();
        
        let pipeline = Self::new(dev, cfgg.clone());

        Ok(pipeline)
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        self.start_pipeline();

        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        self.pause();

        Ok(())
    }

    fn process<'o>(&mut self, clock: &RobotClock, new_msg: &mut Self::Output<'o>) -> CuResult<()> {
        //match self
        //    .appsink
        //    .try_pull_sample(Some(gstreamer::ClockTime::from_useconds(
        //        20,
        //    ))) 
        //{
        //    Some(sample) => {
        //        let buf = sample.buffer().unwrap();
        //        dbg!(sample.caps().unwrap());
        //if let Some(sample) = self.sample.lock().take() {
        //    let buf = sample.buffer().unwrap();
        //    new_msg.set_payload(CuGstBuffer(buf.to_owned()));
        //}

        if let Some(sample) = self.sample_queue.try_recv().ok() {
            let buf = sample.buffer().unwrap();
            new_msg.set_payload(CuGstBuffer(buf.to_owned()));
            println!("wooooo");
        }

        Ok(())
        //        println!("wooooo");
        //        Ok(())
        //    }
        //    None => {
        //        println!("crap");
        //        Ok(())
        //    }
        //}
    }
}
