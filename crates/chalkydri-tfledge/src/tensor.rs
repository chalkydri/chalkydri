use core::alloc::Layout;
use core::mem::size_of;

use crate::Error;
use alloc::alloc::alloc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::ffi::*;

/// Marker trait for tensor input/output types
pub(crate) trait TensorInputOrOutput {}

/// Marker struct for input tensors
pub struct Input;
impl TensorInputOrOutput for Input {}

/// Marker struct for output tensors
pub struct Output;
impl TensorInputOrOutput for Output {}

/// A TensorFlow Lite tensor
///
/// This structure represents a tensor in a TensorFlow Lite model. It can be either an input
/// tensor or an output tensor, depending on the type parameter `IO`.
///
/// # Examples
///
/// ```
/// # use tfledge::{Interpreter, Model, Error, list_devices, Tensor};
/// # fn main() -> Result<(), Error> {
/// # let model = Model::from_file("model.tflite")?;
/// # let device = list_devices().next().unwrap();
/// # let mut interpreter = Interpreter::new(model, device)?;
/// // Get the first input tensor as a tensor of f32 values
/// let input_tensor: Tensor<Input, f32> = interpreter.input_tensor(0);
///
/// // Check the data type of the tensor
/// assert_eq!(input_tensor.kind(), crate::ffi::TfLiteType::kTfLiteFloat32);
/// # Ok(())
/// # }
/// ```
pub struct Tensor<IO, T>
where
    IO: TensorInputOrOutput,
    T: InnerTensorData,
{
    pub(crate) ptr: *mut TfLiteTensor,
    pub(crate) _marker: PhantomData<(IO, T)>,
}
impl<IO, T> Tensor<IO, T>
where
    IO: TensorInputOrOutput,
    T: InnerTensorData,
{
    /// Data type of tensor
    pub fn kind(&self) -> TfLiteType {
        unsafe { TfLiteTensorType(self.ptr) }
    }
    /// Number of dimensions ([None] if opaque)
    pub fn num_dims(&self) -> Option<u32> {
        let i = unsafe { TfLiteTensorNumDims(self.ptr) };

        if i == -1 {
            return None;
        }

        Some(i as u32)
    }
    /// Length of tensor for a given dimension
    pub fn dim(&self, id: u32) -> i32 {
        unsafe { TfLiteTensorDim(self.ptr, id as i32) }
    }
}
// read is lower cost than write
// TODO: Need to specify that here somewhere
impl<T: InnerTensorData> Tensor<Input, T> {
    /// Write data to the tensor.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to write to the tensor.
    ///
    /// # Errors
    ///
    /// Returns an error if the data cannot be written to the tensor.
    pub fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        unsafe {
            let ret = TfLiteTensorCopyFromBuffer(self.ptr, data.as_ptr() as *const _, data.len());

            Error::from(ret)
        }
    }
}
impl<T: InnerTensorData> Tensor<Output, T> {
    /// Read data from the tensor.
    ///
    /// # Type Parameters
    ///
    /// * `const N: usize` - The number of elements to read from the tensor at a time. For
    ///   example, if `N` is 4, then the method will return a vector of 4-element arrays.
    pub fn read<const N: usize>(&self) -> Vec<[T; N]> {
        unsafe {
            // Calculate the number of chunks to read from the tensor
            let ct = TfLiteTensorByteSize(self.ptr) / size_of::<[T; N]>();

            // Get a pointer to the tensor data
            let ptr = TfLiteTensorData(self.ptr);

            // Read the data from the tensor
            core::slice::from_raw_parts::<[T; N]>(ptr as *const _, ct).to_vec()
        }
    }
}

pub(crate) trait InnerTensorData: Clone + Copy + Sized {
    const TFLITE_KIND: TfLiteType;
}

impl InnerTensorData for i8 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteInt8;
}
impl InnerTensorData for i16 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteInt16;
}
impl InnerTensorData for i32 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteInt32;
}
impl InnerTensorData for i64 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteInt64;
}

impl InnerTensorData for u8 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteUInt8;
}
impl InnerTensorData for u16 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteUInt16;
}
impl InnerTensorData for u32 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteUInt32;
}
impl InnerTensorData for u64 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteUInt64;
}

impl InnerTensorData for f32 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteFloat32;
}
impl InnerTensorData for f64 {
    const TFLITE_KIND: TfLiteType = TfLiteType::kTfLiteFloat64;
}

pub struct TensorData<T: InnerTensorData> {
    size: usize,
    ptr: *mut T,
}
impl<T: InnerTensorData> TensorData<T> {
    /// Create a new [`TensorData`] buffer
    ///
    /// # Arguments
    ///
    /// * `size` - The size of the buffer in bytes.
    pub fn new(size: usize) -> Self {
        unsafe {
            let ptr = alloc(Layout::array::<T>(size).unwrap()) as *mut T;

            Self { ptr, size }
        }
    }
    /// Tensor data as a raw slice
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr.cast_const(), self.size) }
    }
}
