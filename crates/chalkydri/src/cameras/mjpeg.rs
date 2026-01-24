use std::{sync::Arc, task::Poll};

use actix_web::web::Bytes;
use futures_core::Stream;
use gstreamer::{
    Caps, Element, ElementFactory, FlowSuccess, Pipeline, glib::object::Cast,
    prelude::GstBinExtManual,
};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use tokio::sync::watch;

use crate::{cameras::preproc::Preprocessor, error::Error, subsystems::NoopSubsys};

// /// Wrapper over frame buffer receiver

/// Preprocessor for driver station MJPEG stream
#[derive(Clone)]
pub struct MjpegProc {
    videorate: Arc<Element>,
    videoconvertscale: Arc<Element>,
    filter: Arc<Element>,
    pub(crate) tx: watch::Sender<Option<Arc<Vec<u8>>>>,
    rx: watch::Receiver<Option<Arc<Vec<u8>>>>,
}
impl Preprocessor for MjpegProc {
    type Subsys = NoopSubsys<Self>;
    type Frame = Vec<u8>;

    fn new(pipeline: &Pipeline) -> Self {
        let videorate = ElementFactory::make("videorate")
            .property("max-rate", 20)
            .property("drop-only", true)
            .build()
            .unwrap();

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

        pipeline
            .add_many([&videorate, &videoconvertscale, &filter])
            .unwrap();

        let (tx, rx) = watch::channel(None);

        MjpegProc {
            videorate: videorate.into(),
            videoconvertscale: videoconvertscale.into(),
            filter: filter.into(),
            tx,
            rx,
        }
    }

    #[tracing::instrument(skip_all)]
    fn link(&self, src: Element, sink: Element) {
        debug!("linking mjpeg preproc");
        Element::link_many([
            &src,
            &self.videorate,
            &self.videoconvertscale,
            &self.filter,
            &sink,
        ])
        .unwrap();
    }

    #[tracing::instrument(skip_all)]
    fn unlink(&self, src: Element, sink: Element) {
        debug!("unlinking mjpeg preproc");
        Element::unlink_many([
            &src,
            &self.videorate,
            &self.videoconvertscale,
            &self.filter,
            &sink,
        ]);
    }

    #[tracing::instrument(skip_all)]
    fn sampler(
        appsink: &AppSink,
        tx: watch::Sender<Option<Arc<Self::Frame>>>,
    ) -> Result<Option<()>, Error> {
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

                while let Err(err) = tx.send(Some(Arc::new(jpeg.to_vec().into()))) {
                    error!("error sending frame: {err:?}");
                }
            }
            None => {
                error!("failed to get buffer");
            }
        }
        Ok(Some(()))
    }
}
impl Stream for MjpegProc {
    type Item = Result<Bytes, Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            match self.rx.has_changed() {
                Ok(true) => {
                    trace!("changed!");
                    let bytes =
                        if let Some(frame) = self.get_mut().rx.borrow_and_update().as_deref() {
                        trace!("got mjpeg frame");
                            [
                                b"--frame\r\nContent-Length: ",
                                frame.len().to_string().as_bytes(),
                                b"\r\nContent-Type: image/jpeg\r\n\r\n",
                                frame,
                            ]
                            .concat()
                        } else {
                            Vec::new()
                        };

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
