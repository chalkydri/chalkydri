use core::ptr::null_mut;

use crate::error::error_reporter;
use crate::ffi::*;
use crate::Error;

use alloc::string::String;

/// A machine learning model which can be loaded onto a device to make inferences
pub struct Model {
    pub(crate) ptr: *mut TfLiteModel,
}
impl Model {
    /// Load a model from a byte slice
    ///
    /// # Arguments
    /// 
    /// * `bytes` - A byte slice containing the raw model data.
    /// 
    /// # Errors
    /// 
    /// Returns an error if the model cannot be loaded from the provided byte slice. This 
    /// usually means the data is invalid or corrupted.
    ///
    /// # Examples
    /// 
    /// ```
    /// # use tfledge::{Model, Error};
    /// # fn main() -> Result<(), Error> {
    /// let model_data: &[u8] = include_bytes!("model.tflite"); // Replace with your model path
    /// let model = Model::from_bytes(model_data)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        unsafe {
            // Create the model, passing in the error reporter
            let ptr = TfLiteModelCreateWithErrorReporter(
                bytes.as_ptr() as *const _,
                bytes.len(),
                Some(error_reporter),
                null_mut(),
            );

            // Check if the model creation was successful
            if ptr.is_null() {
                return Err(Error::FailedToLoadModel);
            }

            Ok(Self { ptr })
        }
    }
    /// Load a model from a file
    ///
    /// # Arguments
    /// 
    /// * `path` - A string slice representing the path to the model file.
    /// 
    /// # Errors
    /// 
    /// Returns an error if the model cannot be loaded from the provided file path. This could 
    /// be due to a file not being found, invalid permissions, or a corrupted model file.
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use tfledge::{Model, Error};
    /// # fn main() -> Result<(), Error> {
    /// let model = Model::from_file("model.tflite")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_file(path: impl Into<String>) -> Result<Self, Error> {
        unsafe {
            // Convert the path to a C string
            let path = path.into();
            let cpath = [path.as_bytes(), b"\0"].concat();

            // Create the model, passing in the error reporter
            let ptr = TfLiteModelCreateFromFileWithErrorReporter(
                cpath.as_ptr() as *const _,
                Some(error_reporter),
                null_mut(),
            );

            // Check if the model creation was successful
            if ptr.is_null() {
                return Err(Error::FailedToLoadModel);
            }

            Ok(Self { ptr })
        }
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        unsafe {
            TfLiteModelDelete(self.ptr);
        }
    }
}
