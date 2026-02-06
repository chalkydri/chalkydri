use chalkydri_apriltags::AprilTagDetections;
use cu29::prelude::*;
use whacknet::{RobotPose, VisionUncertainty};

use crate::comm::{Comm, CommBundleId};

pub struct Resources<'r> {
    pub comm: Borrowed<'r, Comm>,
}
impl<'r> ResourceBindings<'r> for Resources<'r> {
    type Binding = CommBundleId;
    fn from_bindings(
        manager: &'r mut ResourceManager,
        mapping: Option<&ResourceBindingMap<Self::Binding>>,
    ) -> CuResult<Self> {
        let key = mapping
            .expect("comm binding")
            .get(Self::Binding::Comm)
            .expect("comm")
            .typed();
        Ok(Self {
            comm: manager.borrow(key)?,
        })
    }
}

pub struct AprilAdapter {
    cam_id: u64,
    comm: Comm,
}
impl Freezable for AprilAdapter {}
impl CuSinkTask for AprilAdapter {
    type Input<'m> = input_msg!(AprilTagDetections);
    type Resources<'r> = Resources<'r>;

    fn new(config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let cam_id = config.expect("config must be present").get::<u64>("cam_id").expect("cam_id must be set");
        let comm = resources.comm.0.clone();

        Ok(Self {
            cam_id,
            comm,
        })
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, _clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        let det = input.payload().unwrap().clone();

        if let Some(pose) = det.poses.0.first() {
            self.comm.publish(self.cam_id, RobotPose {
                x: pose.translation()[0].value as f64,
                y: pose.translation()[1].value as f64,
                rot: 0.0,
            }, VisionUncertainty::default());
        }

        Ok(())
    }
}
