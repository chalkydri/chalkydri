use std::{fmt::Debug, sync::Arc};

use tokio::sync::{broadcast, watch};

use crate::cameras::CameraManager;

pub type Buffer = Arc<Vec<u8>>;

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
pub trait Subsystem<'fr>: Sized {
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    async fn init(cam_man: &CameraManager) -> Result<Self, Self::Error>;
    /// Process a frame
    async fn process(&mut self) -> Result<Self::Output, Self::Error>;
}

pub struct SubsysHandle<T: Sized> {
    tx: watch::Sender<Buffer>,
    rx: broadcast::Receiver<T>,
}
