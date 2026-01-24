use std::marker::PhantomData;
use std::sync::Arc;

use gstreamer::{Element, ElementFactory, FlowSuccess, Pipeline, prelude::*};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use tokio::sync::watch;

use crate::error::Error;
use crate::subsystems::Subsystem;

/// A set of Gstreamer elements used to preprocess the stream for a [Subsystem]
pub trait Preprocessor {
    type Subsys: Subsystem;
    type Frame: Clone + Send + Sync + 'static;

    /// Initialize the preprocessor
    ///
    /// Gstreamer elements should be created and added to the pipeline here.
    fn init(pipeline: &Pipeline) -> Self;
    /// Link elements to the pipeline
    fn link(&self, src: Element, sink: Element);
    /// Unlink elements from the pipeline
    fn unlink(&self, src: Element, sink: Element);
    /// Callback for new sample event
    /// **Quick** processing of samples and
    #[deprecated]
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

    fn init(_pipeline: &Pipeline) -> Self {
        Self::new()
    }
    fn link(&self, _src: Element, _dst: Element) {}
    fn unlink(&self, _src: Element, _dst: Element) {}
    fn sampler(
        _appsink: &AppSink,
        _tx: watch::Sender<Option<Arc<Self::Frame>>>,
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
        let inner = <P as Preprocessor>::init(pipeline);

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

/// Run frame processing loop
#[deprecated]
pub async fn frame_proc_loop<P: Preprocessor, F: AsyncFnMut(P::Frame) + Sync + Send + 'static>(
    mut rx: watch::Receiver<Option<Arc<P::Frame>>>,
    mut func: F,
) {
    loop {
        'inner: loop {
            match rx.changed().await {
                Ok(()) => match rx.borrow_and_update().clone() {
                    Some(frame) => {
                        if let Some(frame) = Arc::into_inner(frame) {
                            func(frame).await;
                        }
                    }
                    None => {
                        warn!("waiting on first frame...");
                    }
                },
                Err(err) => {
                    error!("error waiting for new frame: {err:?}");
                    break 'inner;
                }
            }
            tokio::task::yield_now().await;
        }
        tokio::task::yield_now().await;
    }
}
