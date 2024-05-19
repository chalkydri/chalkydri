#![no_std]
#![allow(private_bounds)]

extern crate alloc;
extern crate core;

#[macro_use]
extern crate log;

#[allow(nonstandard_style)]
pub(crate) mod ffi {
    include!("gen.rs");
}

mod device;
mod error;
mod interpreter;
mod model;
mod tensor;

pub use device::{list_devices, CoralDevice, CoralDeviceKind, CoralDeviceList};
pub use error::Error;
pub use interpreter::Interpreter;
pub use model::Model;
pub use tensor::{Input, Output, Tensor, TensorData};
