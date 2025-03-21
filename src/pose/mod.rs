use std::{sync::Arc, time::Duration};

use tokio::sync::{Mutex, mpsc};
use transforms::{Registry, Transform, time::Timestamp};

use crate::error::Error;

/// Keeps pose transforms and ...
#[derive(Clone)]
pub struct PoseEstimator {
    /// Transform registry
    reg: Arc<Mutex<Registry>>,
    tx: mpsc::Sender<Transform>,
}
impl PoseEstimator {
    pub fn new() -> Result<Self, Error> {
        let reg = Arc::new(Mutex::new(Registry::new(Duration::from_secs(90))));
        let (tx, mut rx) = mpsc::channel(64);

        let reg_ = reg.clone();
        tokio::spawn(async move {
            while let Some(transform) = rx.recv().await {
                reg_.lock().await.add_transform(transform);
            }
        });

        Ok(Self { reg, tx })
    }

    /// Add a transform to the transform registry
    pub async fn add_transform(&self, transform: Transform) -> Result<(), Error> {
        match self.tx.send(transform).await {
            Ok(_) => Ok(()),
            Err(err) => Err(Error::FailedToAddTransform(err)),
        }
    }

    /// Interpolate the robot's pose based on transforms
    pub async fn get_robot_pose(&self) -> Result<Transform, Error> {
        match self
            .reg
            .lock()
            .await
            .get_transform("robot", "field", Timestamp::now())
        {
            Ok(t) => Ok(t),
            Err(err) => Err(Error::FailedToGetPose(err)),
        }
    }
}
