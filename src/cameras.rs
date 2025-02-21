use gst::prelude::*;
use gstreamer_app::{
    glib::{self, Class},
    gst::{Device, DeviceMonitor, FlowSuccess, Fraction, State},
    AppSink, AppSinkCallbacks,
};
use gstreamer_base::gst::{self, Caps, Pipeline};

#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc};
use tokio::sync::watch;

#[cfg(feature = "rerun")]
use crate::Rerun;
use crate::{
    config::{self, CameraConfig, CameraSettings, CfgFraction},
    Cfg,
};

#[derive(Clone)]
pub struct CameraManager {
    dev_mon: DeviceMonitor,
    pipeline: Pipeline,
}
impl CameraManager {
    pub fn new() -> Self {
        gst::assert_initialized();

        let dev_mon = DeviceMonitor::new();
        dev_mon.add_filter(Some("Video/Source"), None).unwrap();
        dev_mon.start().unwrap();

        let pipeline = Pipeline::new();

        Self { dev_mon, pipeline }
    }
    pub fn devices(&self) -> Vec<config::CameraConfig> {
        self.dev_mon
            .devices()
            .iter()
            .take(1)
            .map(|dev| CameraConfig {
                name: dev.name().to_string(),
                settings: None,
                caps: dev
                    .caps()
                    .unwrap()
                    .iter()
                    .map(|cap| {
                        let frame_rate = cap.get::<Fraction>("framerate").unwrap();
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
            })
            .collect::<Vec<_>>()
    }
    // gamma gamma=2.0 ! fpsdisplaysink ! videorate drop-only=true ! omxh264enc ! mpegtsenc !
    // rtspserversink port=1234
    pub fn load_camera(&self, frame_tx: watch::Sender<Arc<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
        let cam_settings = Cfg.blocking_read().clone();
        let config = CameraConfig { name: String::new(), settings: Some(CameraSettings {
            width: 1280,
            height: 720,
            gamma: None,
            frame_rate: CfgFraction { num: 50, den: 1 },
        }), caps: Vec::new() };
        let cam_settings = cam_settings.cameras.first().unwrap_or(&config);
        let cam_settings = cam_settings.settings.clone().unwrap();

        // Parse pipeline description to create pipeline
        let pipeline = gst::parse::launch(&format!(
            "libcamerasrc ! \
            gamma gamma={} ! \
            capsfilter name=caps_filter caps=video/x-raw,width={},height={},framerate={}/{},format=RGB ! \
            videoconvertscale ! \
            appsink",
            cam_settings.gamma.unwrap_or(1.0),
            cam_settings.width,
            cam_settings.height,
            cam_settings.frame_rate.num,
            cam_settings.frame_rate.den,
        ))
        .unwrap()
        .dynamic_cast::<Pipeline>()
        .unwrap();

        // Get the sink
        let appsink = pipeline
            .iterate_sinks()
            .next()
            .unwrap()
            .unwrap()
            .dynamic_cast::<AppSink>()
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

        // Create the event loop
        let main_loop = glib::MainLoop::new(None, false);
        let main_loop_ = main_loop.clone();
        let bus = pipeline.bus().unwrap();

        // Define the event loop or something?
        bus.connect_message(Some("error"), move |_, msg| match msg.view() {
            gst::MessageView::Error(err) => {
                error!(
                    "error received from element {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                debug!("{:?}", err.debug());

                // Kill event loop
                main_loop_.quit();
            }
            _ => unimplemented!(),
        });
        // idk
        bus.add_signal_watch();

        // Start the pipeline
        pipeline
            .set_state(State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state.");

        // Execute the event loop
        main_loop.run();

        // I think this junk runs if it encounters an error

        pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state.");

        bus.remove_signal_watch();

        Ok(())
    }
}
