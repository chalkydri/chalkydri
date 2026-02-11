use chalkydri_apriltags::AprilTagDetections;
use cu29::prelude::*;
use whacknet::{Comm, CommBundleId, RobotPose, VisionUncertainty};

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
    cam_id: u8,
    comm: Comm,
    last_time: Option<u64>,
}
impl Freezable for AprilAdapter {}
impl CuSinkTask for AprilAdapter {
    type Input<'m> = input_msg!((RobotPose, CuDuration));
    type Resources<'r> = Resources<'r>;

    fn new(config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let cam_id = config
            .expect("config must be present")
            .get::<u8>("cam_id")
            .expect("cam_id must be set");
        let comm = resources.comm.0.clone();

        Ok(Self {
            cam_id,
            comm,
            last_time: None,
        })
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn stop(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        let Tov::Time(time) = input.tov() else {
            return Ok(());
        };
        if let Some((pose, ts)) = input.payload() {
            self.comm.publish(
                self.cam_id,
                0,
                clock.now().as_micros() - time.as_micros(),
                pose.clone(),
                VisionUncertainty::default(),
            );
        }

        Ok(())
    }
}
