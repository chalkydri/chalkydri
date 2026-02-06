use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use cu_gstreamer::CuGstBuffer;
use cu29::prelude::*;
use gstreamer::{
    Caps, ClockTime, Device, Element, ElementFactory, Pipeline, ReferenceTimestampMeta, Sample,
    State, prelude::*,
};
use gstreamer_app::AppSink;

use crate::cameras::providers::{CamProvider, CamProviderBundleId, V4l2Provider};
use chalkydri_core::prelude::*;

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
}
impl CamPipeline {
    /// Create a new camera pipeline from a [Device] and camera config
    pub fn new(dev: Device, cam_config: crate::config::Camera) -> Self {
        let pipeline = Pipeline::new();

        let input = dev.create_element(Some("camera")).unwrap();
        input.set_property("do-timestamp", true);
        {
            input.set_state(State::Ready).unwrap();
            let pad = input.static_pad("src").unwrap();
            let caps = pad.query_caps(None);
            for structure in caps.iter() {
                let structure_name = structure.name();

                // Determine pixel format (handle both raw video and compressed formats)
                let pixel_format = match structure_name.as_str() {
                    "image/jpeg" => "MJPEG".to_string(),
                    "video/x-h264" => "H264".to_string(),
                    "video/x-raw" => structure
                        .get::<String>("format")
                        .unwrap_or_else(|_| "RAW".to_string()),
                    _ => continue, // Skip audio or other non-video streams
                };

                // Extract resolution (skip if reported as ranges rather than fixed values)
                let width: i32 = structure.get("width").ok().unwrap_or(0);
                let height: i32 = structure.get("height").ok().unwrap_or(0);
                if width == 0 || height == 0 {
                    continue; // Skip range-based entries for simplicity
                }

                // Extract framerate (stored as a fraction like 30/1)
                let fps: f64 = structure
                    .get::<gstreamer::Fraction>("framerate")
                    .map(|f| f.numer() as f64 / f.denom() as f64)
                    .unwrap_or(0.0);
                if fps == 0.0 {
                    continue;
                }

                dbg!(width, height, fps, pixel_format,);
            }

            // Clean up: return to NULL state
            let _ = input.set_state(gstreamer::State::Null);
        }

        let settings = cam_config.settings.clone().unwrap_or_default();
        let is_mjpeg = settings.format == Some(String::new());

        let prefilter = ElementFactory::make("capsfilter")
            .name("precapsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
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

        let videoconvert = ElementFactory::make("videoconvert")
            .name("videoconvert")
            .build()
            .unwrap();

        let filter = ElementFactory::make("capsfilter")
            .name("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
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

        pipeline
            .add_many([
                &input,
                &prefilter,
                &videoconvert,
                &filter,
                &jpegdec,
                &videoflip,
                &appsink,
            ])
            .unwrap();

        // If we're getting an MJPEG stream from the cam, it needs to first be decoded
        // if is_mjpeg {
        //     Element::link_many([&input, &jpegdec, &videoflip, &appsink]).unwrap();
        // } else {
        Element::link_many([
            &input,
            &prefilter,
            &videoconvert,
            &filter,
            &videoflip,
            &appsink,
        ])
        .unwrap();

        let appsink = appsink.clone().dynamic_cast::<AppSink>().unwrap();
        appsink.set_sync(false);
        appsink.set_max_buffers(1);
        appsink.set_drop(true);
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

pub struct Resources {
    pub v4l2: Owned<V4l2Provider>,
}
impl<'r> ResourceBindings<'r> for Resources {
    type Binding = CamProviderBundleId;
    fn from_bindings(
        mgr: &'r mut ResourceManager,
        map: Option<&ResourceBindingMap<Self::Binding>>,
    ) -> CuResult<Self> {
        let key = map
            .expect("v4l2 binding")
            .get(Self::Binding::V4L2)
            .expect("v4l2")
            .typed();
        Ok(Self {
            v4l2: mgr.take(key)?,
        })
    }
}

impl Freezable for CamPipeline {}
impl CuSrcTask for CamPipeline {
    type Resources<'r> = Resources;
    type Output<'m> = output_msg!(CuGstBuffer);

    fn new(_config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
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
        if let Some(sample) = self.appsink.try_pull_sample(ClockTime::from_useconds(20)) {
            let buf = sample.buffer().unwrap();
            // Query the configured latency from the pipeline
            let mut query = gstreamer::query::Latency::new();
            if self.pipeline.query(&mut query) {
                let (live, min_latency, max_latency) = query.result();
                println!(
                    "Live: {}, Min latency: {:?}, Max latency: {:?}",
                    live, min_latency, max_latency
                );
            }

            dbg!(
                buf.size(),
                buf.pts(),
                buf.dts(),
                buf.duration(),
                buf.meta::<ReferenceTimestampMeta>(),
                self.pipeline.latency()
            );
            new_msg.set_payload(CuGstBuffer(buf.to_owned()));
            println!("wooooo");
        }

        Ok(())
    }
}
