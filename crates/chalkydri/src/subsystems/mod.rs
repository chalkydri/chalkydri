use std::{fmt::Debug, marker::PhantomData};

use minint::NtConn;
use tokio::sync::watch;

use crate::{cameras::pipeline::Preprocessor, config};

#[cfg(feature = "apriltags")]
pub mod apriltags;
#[cfg(feature = "capriltags")]
pub mod capriltags;
//mod manager;
#[cfg(feature = "ml")]
pub mod ml;
#[cfg(feature = "python")]
pub mod python;

//pub use manager::SubsysManager;

/// A processing subsystem
///
/// Subsystems implement different computer vision tasks, such as AprilTags or object detection.
///
/// A subsystem should be generic, not something that is only used for some specific aspect of a
/// game.
/// For example, note detection for the 2024 game, Crescendo, would go under the object detection
/// subsystem, rather than a brand new subsystem.
///
/// Make sure to pay attention to and respect each subsystem's documentation and structure.
pub trait Subsystem: Sized {
    const NAME: &'static str;

    type Config: Debug + Send + Sync + Clone + 'static;
    type Preproc: Preprocessor;
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    async fn init() -> Result<Self, Self::Error>;

    /// Process a frame
    async fn process(
        &self,
        nt: NtConn,
        cam_config: config::Camera,
        rx: watch::Receiver<Option<<<Self as Subsystem>::Preproc as Preprocessor>::Frame>>,
    ) -> Result<Self::Output, Self::Error>;
}

pub struct NoopSubsys<P: Preprocessor>(PhantomData<P>);
impl<P: Preprocessor> NoopSubsys<P> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<P: Preprocessor> Subsystem for NoopSubsys<P> {
    const NAME: &'static str = "noop";

    type Config = ();
    type Preproc = P;
    type Output = ();
    type Error = ();

    async fn init() -> Result<Self, Self::Error> {
        Ok(Self::new())
    }
    async fn process(
        &self,
        _nt: NtConn,
        _cam_config: config::Camera,
        _rx: watch::Receiver<Option<<<Self as Subsystem>::Preproc as Preprocessor>::Frame>>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(())
    }
}

/// Run frame processing loop
pub async fn frame_proc_loop<P: Preprocessor, F: AsyncFnMut(P::Frame) + Sync + Send + 'static>(
    mut rx: watch::Receiver<Option<P::Frame>>,
    mut func: F,
) {
    loop {
        'inner: loop {
            match rx.changed().await {
                Ok(()) => match rx.borrow_and_update().clone() {
                    Some(frame) => {
                        futures_executor::block_on(async { func(frame).await });
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
        }
        tokio::task::yield_now().await;
    }
}
