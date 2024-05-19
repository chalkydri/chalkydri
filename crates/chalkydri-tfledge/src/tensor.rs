use core::alloc::Layout;
use core::mem::size_of;

use crate::Error;
use alloc::alloc::alloc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::ffi::*;

pub(crate) trait TensorInputOrOutput {}

pub struct Input;
impl TensorInputOrOutput for Input {}

pub struct Output;
impl TensorInputOrOutput for Output {}

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
    pub fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        unsafe {
            let ret = TfLiteTensorCopyFromBuffer(self.ptr, data.as_ptr() as *const _, data.len());

            Error::from(ret)
        }
    }
}
impl<T: InnerTensorData> Tensor<Output, T> {
    // TODO: types other than f32 are possible
    pub fn read<const N: usize>(&self) -> Vec<[T; N]> {
        unsafe {
            let ct = TfLiteTensorByteSize(self.ptr) / size_of::<[T; N]>();

            let ptr = TfLiteTensorData(self.ptr);

            // dim id 1 is a mobilenet-specific thing i think
            // TODO: come back and fix this
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
    pub fn new(size: usize) -> Self {
        unsafe {
            let ptr = alloc(Layout::array::<T>(size).unwrap()) as *mut T;

            Self { ptr, size }
        }
    }
    /// Tensor data as raw bytes
    pub fn as_bytes(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr.cast_const(), self.size) }
    }
}
