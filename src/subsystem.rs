use std::{fmt::Debug, sync::Arc};

use rerun::RecordingStream;
use tokio::sync::{broadcast, watch};

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
    /// The actual frame processing [Actor]
    ///
    /// May be `Self`
    //type Processor: Actor + Handler<ProcessFrame<Self::Output, Self::Error>>;
    /// The subsystem's configuration type
    type Config;
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    async fn init(cfg: Self::Config) -> Result<Self, Self::Error>;
    /// Process a frame
    fn process(&mut self, buf: Buffer, rr: RecordingStream) -> Result<Self::Output, Self::Error>;
}

/// Run a [`subsystem`](Subsystem)
async fn run<'fr, S: Subsystem<'fr>>(
    config: S::Config,
    mut rx: watch::Receiver<Arc<Vec<u8>>>,
    rr: RecordingStream,
) {
    let mut subsys = S::init(config).await.unwrap();

    while let Ok(()) = rx.changed().await {
        let buf = rx.borrow_and_update();
        S::process(&mut subsys, buf.clone(), rr.clone()).unwrap();
    }
}

pub struct SubsysHandle<T: Sized> {
    tx: watch::Sender<Buffer>,
    rx: broadcast::Receiver<T>,
}
