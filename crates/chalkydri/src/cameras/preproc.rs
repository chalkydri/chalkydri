use std::marker::PhantomData;
use std::sync::Arc;

use gstreamer::{Element, ElementFactory, FlowSuccess, Pipeline, prelude::*};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use tokio::sync::watch;

use chalkydri_core::prelude::*;

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

        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    trace!("got sample");
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
