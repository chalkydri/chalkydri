use std::{fmt::Debug, sync::Arc};

use gstreamer::{Buffer, BufferRef, SampleRef};
use tokio::sync::{broadcast, watch};

use crate::{cameras::CameraManager, config};

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
pub trait Subsystem<'fr>: Sized {
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    fn init(cam_config: &config::Camera) -> Result<Self, Self::Error>;
    /// Process a frame
    fn process(&mut self, frame: Buffer) -> Result<Self::Output, Self::Error>;
}

pub struct SubsysHandle<T: Sized> {
    tx: watch::Sender<Buffer>,
    rx: broadcast::Receiver<T>,
}
