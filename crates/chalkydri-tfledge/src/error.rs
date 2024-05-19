use crate::ffi::*;

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Error,
    DelegateError,
    UnresolvedOps,
    ApplicationError,
    DelegateDataNotFound,
    DelegateDataReadError,
    DelegateDataWriteError,
    FailedToCreateInterpreter,
    FailedToLoadModel,
}
impl Error {
    pub(crate) fn from(ffi: TfLiteStatus) -> Result<(), Self> {
        match ffi {
            TfLiteStatus::kTfLiteOk => Ok(()),
            TfLiteStatus::kTfLiteCancelled => Err(Self::Cancelled),
            TfLiteStatus::kTfLiteError => Err(Self::Error),
            TfLiteStatus::kTfLiteDelegateError => Err(Self::DelegateError),
            TfLiteStatus::kTfLiteUnresolvedOps => Err(Self::UnresolvedOps),
            TfLiteStatus::kTfLiteApplicationError => Err(Self::ApplicationError),
            TfLiteStatus::kTfLiteDelegateDataNotFound => Err(Self::DelegateDataNotFound),
            TfLiteStatus::kTfLiteDelegateDataReadError => Err(Self::DelegateDataReadError),
            TfLiteStatus::kTfLiteDelegateDataWriteError => Err(Self::DelegateDataWriteError),
        }
    }
}

pub(crate) extern "C" fn error_reporter(
    _data: *mut core::ffi::c_void,
    format: *const core::ffi::c_char,
    args: *mut __va_list_tag,
) {
    unsafe {
        let mut buf = [0i8; 512];

        vsnprintf(buf.as_mut_ptr(), 512, format, args);

        let s = core::str::from_utf8_unchecked(core::ffi::CStr::from_ptr(buf.as_ptr()).to_bytes());

        error!(target: "tfledge::tflitec", "{s}");
    }
}
