use core::ptr::null_mut;

use crate::error::error_reporter;
use crate::tensor::InnerTensorData;
use crate::{CoralDevice, Error, Input, Model, Output, Tensor};
use core::marker::PhantomData;

use crate::ffi::*;

/// The core structure for making inferences with TFLite
///
/// ```
/// let mut int = Interpreter::new(model, device)?;
///
/// int.invoke()?;
/// ```
pub struct Interpreter {
    ptr: *mut TfLiteInterpreter,
}
impl Interpreter {
    /// Create a new [Interpreter] and allocate tensors for a given [Model]
    pub fn new(model: Model, dev: CoralDevice) -> Result<Self, Error> {
        unsafe {
            // Build the interpreter options
            let opts: *mut TfLiteInterpreterOptions = TfLiteInterpreterOptionsCreate();
            TfLiteInterpreterOptionsSetErrorReporter(opts, Some(error_reporter), null_mut());
            TfLiteInterpreterOptionsAddDelegate(opts, dev.create_delegate());

            // Create the interpreter
            let ptr = TfLiteInterpreterCreate(model.ptr, opts);
            if ptr.is_null() {
                return Err(Error::FailedToCreateInterpreter);
            }

            // Allocate tensors
            Self::allocate_tensors(&mut Self { ptr })?;

            Ok(Self { ptr })
        }
    }

    /// Allocate tensors for the interpreter
    fn allocate_tensors(&mut self) -> Result<(), Error> {
        unsafe {
            let ret = TfLiteInterpreterAllocateTensors(self.ptr);
            Error::from(ret)
        }
    }

    /// Get an input tensor
    pub fn input_tensor<T: InnerTensorData>(&self, id: u32) -> Tensor<Input, T> {
        unsafe {
            let ptr = TfLiteInterpreterGetInputTensor(self.ptr, id as i32);

            Tensor::<Input, T> {
                ptr,
                _marker: PhantomData,
            }
        }
    }

    /// Get an output tensor
    pub fn output_tensor<T: InnerTensorData>(&self, id: u32) -> Tensor<Output, T> {
        unsafe {
            let ptr = TfLiteInterpreterGetOutputTensor(self.ptr, id as i32);

            Tensor::<Output, T> {
                ptr: ptr as *mut _,
                _marker: PhantomData,
            }
        }
    }

    /// Run inference
    ///
    /// This basically just processes data from the input tensors, using the model, into the ourput
    /// tensors.
    pub fn invoke(&mut self) -> Result<(), Error> {
        unsafe {
            let ret = TfLiteInterpreterInvoke(self.ptr);

            Error::from(ret)
        }
    }
}
impl Drop for Interpreter {
    fn drop(&mut self) {
        unsafe {
            TfLiteInterpreterDelete(self.ptr);
        }
    }
}
