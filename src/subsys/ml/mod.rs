//!
//! Machine learning subsystem
//!

use std::io::{Read, Write};

use crate::Subsystem;
use tfledge::Model;

use self::model::Model;

pub struct MlSubsys<'subsys> {
    int: Interpreter<'subsys>,
}
impl<'subsys> Subsystem<'subsys> for MlSubsys<'subsys> {
    fn init() -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let m = Model::from_file("Note_Detector.tflite");

        let d = tfledge::list_devices().next().unwrap();
    }
    fn run(&self, rt: tokio::runtime::Runtime) {
        let _g = rt.enter();
    }
}
