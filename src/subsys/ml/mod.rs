//!
//! Machine learning subsystem
//!

use crate::{ProcessFrame, Subsystem};
use actix::prelude::*;
use tfledge::{CoralDevice, Error, Input, Interpreter, Model, Output, Tensor};

/// The machine learning subsystems processor actor
pub struct MlInterpreter {
    int: Interpreter,
}
impl Actor for MlInterpreter {
    type Context = Context<Self>;
}
impl Handler<ProcessFrame<'_, Tensor<Output, f32>>> for MlInterpreter {
    type Result = Result<Tensor<Output, f32>, Box<dyn std::error::Error>>;

    fn handle(
        &mut self,
        msg: ProcessFrame<'_, Tensor<Output, f32>>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        // Set up the input tensors

        self.int.invoke().expect("failed to invoke the interpreter");

        // Do junk with the output tensors

        Ok(self.int.output_tensor(0))
    }
}

pub struct MlSubsysCfg {
    pub model_path: String,
}

/// Chalkydri's machine learning subsystem
///
/// This uses [::tfledge] to interact with Coral devices through TensorFlow Lite.
pub struct MlSubsys {
    dev: CoralDevice,
}
impl Subsystem<'_, Tensor<Output, f32>> for MlSubsys {
    type Processor = MlInterpreter;
    type Config = MlSubsysCfg;

    async fn init() -> Result<Self, Box<dyn std::error::Error>> {
        // Get the first available Coral device
        let dev = tfledge::list_devices()
            .next()
            .expect("no Coral devices found");

        Ok(Self { dev })
    }
    async fn run(self, cfg: Self::Config) -> Addr<Self::Processor> {
        // Load TFLite model from the path in the config
        let model = Model::from_file(cfg.model_path.as_str()).unwrap();

        let int = Interpreter::new(model, self.dev).unwrap();

        MlInterpreter { int }.start()
    }
}
