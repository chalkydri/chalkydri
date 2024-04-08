use core::ptr::null_mut;

use crate::ffi::*;

/// Coral device type
#[derive(PartialEq, Eq)]
pub enum CoralDeviceKind {
    Usb,
    Pci,
}
impl CoralDeviceKind {
    fn from(ffi: edgetpu_device_type) -> Self {
        match ffi {
            edgetpu_device_type::EDGETPU_APEX_USB => Self::Usb,
            edgetpu_device_type::EDGETPU_APEX_PCI => Self::Pci,
        }
    }
}

/// A Coral device
pub struct CoralDevice {
    ptr: edgetpu_device,
}
impl CoralDevice {
    pub fn kind(&self) -> CoralDeviceKind {
        CoralDeviceKind::from(self.ptr.type_)
    }
    pub fn path(&self) -> &str {
        unsafe { core::ffi::CStr::from_ptr(self.ptr.path).to_str().unwrap() }
    }

    pub(crate) fn create_delegate(&self) -> *mut TfLiteDelegate {
        unsafe {
            let opts: *mut edgetpu_option = null_mut();

            let ptr = edgetpu_create_delegate(self.ptr.type_, self.ptr.path, opts, 0);
            if ptr.is_null() {
                panic!();
            }

            ptr
        }
    }
}

pub struct CoralDeviceList {
    ptr: *mut edgetpu_device,
    ct: usize,
    curr: usize,
}
impl Iterator for CoralDeviceList {
    type Item = CoralDevice;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let res: Option<Self::Item>;

            if self.curr < self.ct {
                let devs = core::slice::from_raw_parts(self.ptr, self.ct);
                res = Some(CoralDevice {
                    ptr: devs[self.curr],
                });
            } else {
                res = None;
            }
            self.curr += 1;
            return res;
        }
    }
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.ct
    }
}
impl Drop for CoralDeviceList {
    fn drop(&mut self) {
        unsafe {
            edgetpu_free_devices(self.ptr);
        }
    }
}

/// List available [`Coral Edge TPU devices`](CoralDevice)
pub fn list_devices() -> CoralDeviceList {
    unsafe {
        let mut ct: usize = 0;
        let ptr = edgetpu_list_devices(&mut ct);

        CoralDeviceList { ptr, ct, curr: 0 }
    }
}
