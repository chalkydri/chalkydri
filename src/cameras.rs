use gst::prelude::*;
use gstreamer_app::{
    glib,
    gst::{FlowSuccess, State},
    AppSink, AppSinkCallbacks,
};
use gstreamer_base::gst::{self, Caps, Pipeline};

#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{error::Error, sync::Arc};
use tokio::sync::watch;

#[cfg(feature = "rerun")]
use crate::Rerun;

pub fn load_cameras(frame_tx: watch::Sender<Arc<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
    // Initialize gstreamer
    gst::init()?;

    // Parse pipeline description to create pipeline
    let pipeline = gst::parse::launch(
        "libcamerasrc ! \
            capsfilter caps=video/x-raw,width=1280,height=720 ! \
            videoconvertscale ! \
            appsink",
    )
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
