use core::ptr::null_mut;

use crate::error::error_reporter;
use crate::tensor::InnerTensorData;
use crate::{CoralDevice, Error, Input, Model, Output, Tensor};
use core::marker::PhantomData;

use crate::ffi::*;

/// The core structure for making inferences with TFLite
///
/// # Examples
///
/// ```
/// # use tfledge::{Interpreter, Model, Error, list_devices};
/// # fn main() -> Result<(), Error> {
/// // Load the TFLite model
/// let model = Model::from_file("model.tflite")?;
///
/// // Get a Coral device
/// let device = list_devices().next().unwrap();
///
/// // Create a new interpreter
/// let mut interpreter = Interpreter::new(model, device)?;
///
/// // ... perform inference ...
///
/// # Ok(())
/// # }
/// ```
pub struct Interpreter {
    ptr: *mut TfLiteInterpreter,
}
impl Interpreter {
    /// Create a new [Interpreter] and allocate tensors for a given [Model]
    ///
    /// # Arguments
    ///
    /// * `model` - The TensorFlow Lite model to use for inference.
    /// * `dev` - The Coral device to use for acceleration.
    ///
    /// # Errors
    ///
    /// Returns an error if the interpreter cannot be created or if the tensors cannot be
    /// allocated.
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
    ///
    /// This method is called by [`Interpreter::new`] to allocate the tensors required by the
    /// model.
    fn allocate_tensors(&mut self) -> Result<(), Error> {
        unsafe {
            let ret = TfLiteInterpreterAllocateTensors(self.ptr);
            Error::from(ret)
        }
    }

    /// Get an input tensor
    ///
    /// # Arguments
    ///
    /// * `id` - The index of the input tensor to get.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The data type of the tensor. Must implement [`InnerTensorData`].
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
    ///
    /// # Arguments
    ///
    /// * `id` - The index of the output tensor to get.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The data type of the tensor. Must implement [`InnerTensorData`].
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
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
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
