use actix::prelude::*;

use crate::{ProcessFrame, Subsystem};

/// Configuration for the AprilTags subsystem
pub struct ApriltagsConfig {
    pub workers: usize,
}

/// The AprilTags subsystem
#[derive(Clone)]
pub struct Apriltags {
    det: (),
}
impl Subsystem<'_, (), ()> for Apriltags {
    type Processor = Self;
    type Config = ApriltagsConfig;

    async fn init() -> Result<Self, ()> {
        Ok(Self { det: () })
    }
    async fn run(self, cfg: Self::Config) -> actix::Addr<Self::Processor> {
        SyncArbiter::start(cfg.workers, move || self.clone())
    }
}
impl Actor for Apriltags {
    type Context = SyncContext<Self>;
}
impl Handler<ProcessFrame<(), ()>> for Apriltags {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ProcessFrame<(), ()>, ctx: &mut Self::Context) -> Self::Result {
        //

        Ok(())
    }
}
