use std::sync::Arc;

use futures_util::StreamExt;
use gstreamer::{Caps, Element, ElementFactory, FlowSuccess, Pipeline, prelude::*};
use gstreamer_app::{AppSink, AppSinkCallbacks, app_sink::AppSinkStream};
use tokio::sync::watch;

use chalkydri_core::prelude::*;

/// Wrapper around [Preprocessor] implementations that handles the [AppSink] junk
#[derive(Clone)]
pub struct PreprocWrap<P: SubsysPreprocessor<Frame = Vec<u8>>> {
    inner: P,
    appsink: Element,
    tx: watch::Sender<Option<Arc<P::Frame>>>,
    rx: watch::Receiver<Option<Arc<P::Frame>>>,
}
impl<P: SubsysPreprocessor<Frame = Vec<u8>>> PreprocWrap<P> {
    /// Create a new wrapped preprocessor
    pub fn new(pipeline: &Pipeline) -> Self {
        let inner = P::init(pipeline);

        let appsink = ElementFactory::make("appsink").build().unwrap();

        if let Err(err) = pipeline.add(&appsink) {
            error!("failed to add appsink to pipeline: {err:?}");
        }

        let (tx, rx) = watch::channel(None);

        Self {
            inner,
            appsink,
            tx,
            rx,
        }
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

        //let stream = std::pin::pin!(appsink.stream());

        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    trace!("got sample");
                    let sample = appsink.pull_sample().unwrap();
                    let buf = sample.buffer().unwrap();
                    let buf = buf
                        .to_owned()
                        .into_mapped_buffer_readable()
                        .unwrap()
                        .to_vec();
                    tx.send(Some(Arc::new(buf))).unwrap();
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
    pub fn rx(&self) -> AppSinkStream {
        Element::dynamic_cast::<AppSink>(self.appsink.clone())
            .unwrap()
            .stream()
    }
}
