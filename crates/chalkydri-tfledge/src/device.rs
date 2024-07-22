use core::ptr::null_mut;

use crate::ffi::*;

/// Represents the type of Coral device.
#[derive(PartialEq, Eq)]
pub enum CoralDeviceKind {
    /// A Coral device connected via USB.
    Usb,
    /// A Coral device installed on a PCI slot.
    Pci,
}

impl CoralDeviceKind {
    /// Converts the FFI representation of device type to the `CoralDeviceKind` enum.
    fn from(ffi: edgetpu_device_type) -> Self {
        match ffi {
            edgetpu_device_type::EDGETPU_APEX_USB => Self::Usb,
            edgetpu_device_type::EDGETPU_APEX_PCI => Self::Pci,
        }
    }
}

/// Represents a Coral Edge TPU device.
///
/// This struct provides information about a detected Coral device, including its type and system path.
/// You'll primarily use this struct to:
///
/// - Identify the type of Coral device connected (USB or PCI).
/// - Retrieve the system path to the device, which can be used for further interactions with the device.
///
/// # Examples
///
/// ```
/// use tfledge::{list_devices, CoralDeviceKind};
///
/// let devices = list_devices();
///
/// for device in devices {
///     // Print the path of the device
///     println!("Device path: {}", device.path());
///     
///     // Check the kind of device
///     match device.kind() {
///         CoralDeviceKind::Usb => println!("This is a USB Coral device."),
///         CoralDeviceKind::Pci => println!("This is a PCI Coral device."),
///     }
/// }
/// ```
pub struct CoralDevice {
    ptr: edgetpu_device,
}

impl CoralDevice {
    /// Returns the type of the Coral device.
    ///
    /// # Examples
    ///
    /// See the example in [`CoralDevice`].
    pub fn kind(&self) -> CoralDeviceKind {
        CoralDeviceKind::from(self.ptr.type_)
    }

    /// Returns the system path of the Coral device.
    ///
    /// # Examples
    ///
    /// See the example in [`CoralDevice`].
    pub fn path(&self) -> &str {
        unsafe { core::ffi::CStr::from_ptr(self.ptr.path).to_str().unwrap() }
    }

    /// Creates a new `TfLiteDelegate` for this device.
    ///
    /// This method is used internally to set up the Coral device for use with a TensorFlow Lite interpreter.
    pub(crate) fn create_delegate(&self) -> *mut TfLiteDelegate {
        unsafe {
            // Use default options for the delegate
            let opts: *mut edgetpu_option = null_mut();

            // Create the delegate using the FFI function
            let ptr = edgetpu_create_delegate(self.ptr.type_, self.ptr.path, opts, 0);

            // Panic if the delegate creation fails.
            // In a real application, you'd want to handle this error gracefully.
            if ptr.is_null() {
                panic!("Failed to create Edge TPU delegate.");
            }

            ptr
        }
    }
}

/// An iterator over available Coral Edge TPU devices.
///
/// This struct is returned by the [`list_devices()`] function and lets you iterate over all
/// detected Coral devices. You can use this iterator to find a specific device or to list
/// all available devices.
///
/// # Examples
///
/// ```
/// use tfledge::{list_devices, CoralDeviceKind};
///
/// // Iterate over each detected Coral device
/// for device in list_devices() {
///     println!("Found Coral device: {}", device.path());
/// }
/// ```
pub struct CoralDeviceList {
    ptr: *mut edgetpu_device,
    ct: usize,
    curr: usize,
}

impl Iterator for CoralDeviceList {
    type Item = CoralDevice;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            // Check if we're past the end of the device list
            if self.curr >= self.ct {
                return None;
            }

            // Get a reference to the current device from the raw pointer
            let device = self.ptr.add(self.curr);

            // Increment the counter to the next device
            self.curr += 1;

            // Construct and return a `CoralDevice` from the raw pointer.
            Some(CoralDevice { ptr: *device })
        }
    }
}

impl Drop for CoralDeviceList {
    fn drop(&mut self) {
        // Free the memory allocated for the device list by the FFI
        unsafe {
            edgetpu_free_devices(self.ptr);
        }
    }
}

/// Lists all available Coral Edge TPU devices.
///
/// This function returns a [`CoralDeviceList`], which can be used to iterate over all detected
/// Coral devices.
///
/// # Examples
///
/// ```
/// use tfledge::list_devices;
///
/// for device in list_devices() {
///     println!("Found Coral device: {}", device.path());
/// }
/// ```
pub fn list_devices() -> CoralDeviceList {
    unsafe {
        let mut ct: usize = 0;

        // Get a pointer to the list of devices and the number of devices
        let ptr = edgetpu_list_devices(&mut ct);

        // Construct and return the CoralDeviceList
        CoralDeviceList { ptr, ct, curr: 0 }
    }
}
