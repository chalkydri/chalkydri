use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use nt_client::ClientHandle as NTClientHandle;
use tokio::sync::watch;

use crate::{config, preprocs::Preprocessor};

/// Subsystem control message
#[derive(Clone)]
pub enum SubsystemCtrl {
    //<S: Subsystem> {
    Start,
    Stop,
    //ConfigUpdate(S::Config),
    CamUpdate(config::Camera),
}

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
    /// Whether the subsystem is required to stay on the same thread
    const THREAD_LOCAL: bool = false;

    type Config: Debug + Send + Sync + Clone + 'static;
    type Preproc: Preprocessor;
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    async fn init(nt: &NTClientHandle, cam_config: config::Camera) -> Result<Self, Self::Error>;

    /// Process a frame
    async fn process(
        &self,
        nt: &NTClientHandle,
        cam_config: config::Camera,
        frame: Arc<Vec<u8>>,
    ) -> Result<Self::Output, Self::Error>;

    /// Do anything that may be required to shut down the subsystem
    ///
    /// The implementor's [Drop] implementation will be called as well.
    fn stop(&mut self) {}
}

/// A subsystem that does nothing
///
/// This can be used to run a [Preprocessor] without running a subsystem
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

    async fn init(_nt: &NTClientHandle, _cam_config: config::Camera) -> Result<Self, Self::Error> {
        Ok(Self::new())
    }
    async fn process(
        &self,
        _nt: &NTClientHandle,
        _cam_config: config::Camera,
        _rx: Arc<Vec<u8>>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(())
    }
}
