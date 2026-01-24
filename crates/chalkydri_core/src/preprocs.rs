use std::marker::PhantomData;
use std::sync::Arc;

use futures_core::stream::Stream;
use gstreamer::{Element, ElementFactory, FlowSuccess, Pipeline, Sample, prelude::*};
use gstreamer_app::app_sink::AppSinkStream;
use gstreamer_app::{AppSink, AppSinkCallbacks};
use tokio::sync::watch;

use crate::error::Error;
use crate::subsystems::Subsystem;

/// A set of Gstreamer elements used to preprocess the stream for a [Subsystem]
pub trait SubsysPreprocessor {
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
}

/// A no-op preprocessor for subsystems that don't require any preprocessing
pub struct NoopPreproc<S: Subsystem>(PhantomData<S>);
impl<S: Subsystem> NoopPreproc<S> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<S: Subsystem> SubsysPreprocessor for NoopPreproc<S> {
    type Subsys = S;
    type Frame = ();

    fn init(_pipeline: &Pipeline) -> Self {
        Self::new()
    }
    fn link(&self, _src: Element, _dst: Element) {}
    fn unlink(&self, _src: Element, _dst: Element) {}
}

/// Run frame processing loop
#[deprecated]
pub async fn frame_proc_loop<P: SubsysPreprocessor, F: AsyncFnMut(P::Frame) + Sync + Send + 'static>(
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
