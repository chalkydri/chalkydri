//!
//! Machine learning subsystem
//!

use crate::{ProcessFrame, Subsystem, Cfg};
use actix::prelude::*;
use tfledge::{CoralDevice, Error, Input, Interpreter, Model, Output, Tensor};

/// The machine learning subsystems processor actor
pub struct MlInterpreter {
    int: Interpreter,
}
impl<'fr> Subsystem<'fr> for MlInterpreter {
    async fn init() -> Result<Self, Self::Error> {
        // Get the first available Coral device
        let dev = tfledge::list_devices()
            .next()
            .expect("no Coral devices found"); 

        // Load TFLite model from the path in the config
        let model = Model::from_file(Cfg.model_path.as_str()).unwrap();

        let int = Interpreter::new(model, dev).unwrap();
        
        Self { int }
    }

    fn process(&mut self, buf: crate::subsystem::Buffer) -> Result<Self::Output, Self::Error> {
        // Set up the input tensors

        self.int.invoke().expect("failed to invoke the interpreter");

        // Do junk with the output tensors

        Ok(self.int.output_tensor(0))
    }
    type Output = Tensor<Output, f32>;
    type Error = Box<dyn std::error::Error + Send>;
}
