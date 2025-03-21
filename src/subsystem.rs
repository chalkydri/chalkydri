use std::fmt::Debug;

use gstreamer::{Buffer, Element, Pipeline};
use minint::NtConn;
use tokio::sync::watch;

use crate::config;

//pub type Buffer = Arc<Vec<u8>>;

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
    async fn init(cam_config: config::Camera) -> Result<Self, Self::Error>;

    /// Initialize the subsystem's preprocessing pipeline chunk
    fn preproc(
        config: config::Camera,
        pipeline: &Pipeline,
    ) -> Result<(Element, Element), Self::Error>;

    /// Process a frame
    async fn process(
        &mut self,
        nt: NtConn,
        rx: watch::Receiver<Option<Buffer>>,
    ) -> Result<Self::Output, Self::Error>;
}

pub async fn frame_proc_loop(
    mut rx: watch::Receiver<Option<Buffer>>,
    mut func: impl AsyncFnMut(Buffer),
) {
    loop {
        'inner: loop {
            match rx.changed().await {
                Ok(()) => match rx.borrow_and_update().clone() {
                    Some(frame) => {
                        func(frame).await;
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
