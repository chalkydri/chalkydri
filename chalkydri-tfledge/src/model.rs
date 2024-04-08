use core::ptr::null;
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
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        unsafe {
            // TODO: Use the error reporter functionality
            //let ptr = TfLiteModelCreate(bytes.as_ptr() as *const _, bytes.len());
            let ptr = TfLiteModelCreateWithErrorReporter(
                bytes.as_ptr() as *const _,
                bytes.len(),
                Some(error_reporter),
                null_mut(),
            );
            if ptr.is_null() {
                return Err(Error::FailedToLoadModel);
            }

            Ok(Self { ptr })
        }
    }
    /// Load a model from a file
    pub fn from_file(path: impl Into<String>) -> Result<Self, Error> {
        unsafe {
            let path = path.into();

            let cpath = [path.as_bytes(), b"\0"].concat();

            // TODO: Use the error reporter functionality
            let ptr = TfLiteModelCreateFromFileWithErrorReporter(
                cpath.as_ptr() as *const _,
                Some(error_reporter),
                null_mut(),
            );
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
