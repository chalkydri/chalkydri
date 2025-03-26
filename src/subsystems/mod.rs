use std::fmt::Debug;

use gstreamer::{Buffer, Element, Pipeline};
use minint::NtConn;
use tokio::sync::watch;

use crate::config;

#[cfg(feature = "apriltags")]
pub mod apriltags;
#[cfg(feature = "capriltags")]
pub mod capriltags;
mod manager;
#[cfg(feature = "ml")]
pub mod ml;
#[cfg(feature = "python")]
pub mod python;

pub use manager::SubsysManager;

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
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    async fn init() -> Result<Self, Self::Error>;

    /// Initialize the subsystem's preprocessing pipeline chunk
    fn preproc(
        config: config::Camera,
        pipeline: &Pipeline,
    ) -> Result<(Element, Element), Self::Error>;

    /// Process a frame
    async fn process(
        &self,
        manager: SubsysManager,
        nt: NtConn,
        cam_config: config::Camera,
        rx: watch::Receiver<Option<Vec<u8>>>,
    ) -> Result<Self::Output, Self::Error>;
}

pub async fn frame_proc_loop<F: AsyncFnMut(Vec<u8>) + Sync + Send + 'static>(
    mut rx: watch::Receiver<Option<Vec<u8>>>,
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
