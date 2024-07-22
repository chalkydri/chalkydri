//!
//! Machine learning subsystem
//!

use std::io::{Read, Write};

use crate::Subsystem;
use tfledge::{Interpreter, Model};


pub struct MlSubsys {
    int: Interpreter,
}
impl Subsystem for MlSubsys {
    async fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let m = Model::from_file("Note_Detector.tflite").unwrap();

        let d = tfledge::list_devices().next().unwrap();

        Ok(Self { int: Interpreter::new(m, d).unwrap() })
    }
    async fn run(&self) {
        //
    }
    async fn shutdown(self) {
        //
    }
}
