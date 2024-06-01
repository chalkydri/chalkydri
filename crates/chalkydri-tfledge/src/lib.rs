//!
//! # TFLedge
//!
//! Safe wrapper around TensorFlow Lite and libedgetpu
//!
//! This is used to interact with Coral edge TPUs and do ***\****machine learning***\**** and ***\****AI***\****.
//!
//! ```
//! #use std::{fs::File, io::Read, time::Instant};
//! 
//! #use tfledge::{list_devices, Error, Interpreter, Model};
//! 
//! #fn main() -> Result<(), Error> {
//!     // Load the TFLite model from a file
//!     let m = Model::from_file("Note_Detector.tflite")?;
//! 
//!     // Get the first edge TPU device
//!     let d = list_devices().next().unwrap();
//!     // Build an interpreter
//!     let mut int = Interpreter::new(m, d).unwrap();
//! 
//!     // Get a handle to input tensor 0
//!     let mut input = int.input_tensor::<f32>(0);
//! 
//!     assert_eq!(input.num_dims().unwrap(), 4);
//! 
//!     let mut buf: Vec<u8> = Vec::new();
//!     File::open("test.rgb")
//!         .unwrap()
//!         .read_to_end(&mut buf)
//!         .unwrap();
//!     input.write(&buf).unwrap();
//! 
//!     println!("{}", input.num_dims().unwrap());
//!     for dim in 0..input.num_dims().unwrap() {
//!         println!("- {}", input.dim(dim));
//!     }
//! 
//!     for _ in 0..100_000 {
//!         let st = Instant::now();
//!         int.invoke()?;
//! 
//!         let boxes = int.output_tensor::<f32>(1).read::<4>();
//!         let classes = int.output_tensor::<f32>(3).read::<1>();
//!         let scores = int.output_tensor::<f32>(0).read::<1>();
//! 
//!         for aaa in boxes {
//!             println!("{aaa:?}");
//!         }
//! 
//!         for aaa in classes {
//!             println!("{aaa:?}");
//!         }
//! 
//!         for aaa in scores {
//!             println!("{aaa:?}");
//!         }
//! 
//!         /*
//!         for (label, output, chunksz) in [
//!             ("boxes", int.output_tensor(1), 4),
//!             ("classes", int.output_tensor(3), 1),
//!             ("scores", int.output_tensor(0), 1),
//!         ] {
//!             println!("[{label}] ({:?})", output.kind());
//!             println!("{}", output.num_dims());
//!             for dim in 0..output.num_dims() {
//!                 println!("- {}", output.dim(dim));
//!             }
//! 
//!             for aaa in output.read::<chunksz>() {
//!                 println!("{aaa:?}");
//!             }
//!         }
//!         */
//!         println!("{:?}", st.elapsed());
//!     }
//! 
//!     Ok(())
//! }
//! ```
//!

#![no_std]
#![allow(private_bounds)]

extern crate alloc;
extern crate core;

#[macro_use]
extern crate log;

#[allow(nonstandard_style, dead_code)]
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
