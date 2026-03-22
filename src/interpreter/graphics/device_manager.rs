//! Device manager — tracks all open graphics devices on the interpreter.
//!
//! R uses 1-based device numbering where device 1 is always the "null device".
//! The `DeviceManager` maintains a vector of device slots and tracks which
//! device is currently active. When the last real device is closed, the
//! current device reverts to 1 (the null device).

use crate::interpreter::graphics::GraphicsDevice;
use crate::interpreter::value::{RError, RErrorKind};

/// Manages all open graphics devices for an interpreter instance.
///
/// Devices are stored in a 1-indexed `Vec<Option<...>>` where index 0 is
/// unused (padding) and index 1 is reserved for the null device concept.
/// Real devices start at index 2.
pub struct DeviceManager {
    /// Device slots. Index 0 is unused padding; index 1 is the null device;
    /// indices 2+ are user-opened devices. `None` means the slot is empty
    /// (device was closed).
    devices: Vec<Option<Box<dyn GraphicsDevice>>>,
    /// The currently active device (1-indexed). 1 = null device (no real
    /// device open).
    current: usize,
}

impl DeviceManager {
    /// Create a new device manager with no open devices.
    ///
    /// The null device (index 1) is always conceptually present but is not
    /// stored as an actual device — it is represented by `current == 1` when
    /// no real devices are open.
    pub fn new() -> Self {
        DeviceManager {
            // Index 0 = unused padding, index 1 = null device slot (None,
            // since the null device is implicit)
            devices: vec![None, None],
            current: 1,
        }
    }

    /// Return the 1-based index of the currently active device.
    ///
    /// Returns 1 (null device) when no real devices are open.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Switch the active device to `n`. Returns the previous active device index.
    ///
    /// Device 1 (null device) is always valid. Other indices must refer to
    /// an open device.
    pub fn set_current(&mut self, n: usize) -> Result<usize, RError> {
        if n == 1 {
            let prev = self.current;
            self.current = 1;
            return Ok(prev);
        }

        if n == 0 || n >= self.devices.len() || self.devices[n].is_none() {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "dev.set({n}): no device at index {n}\n  \
                     Use dev.list() to see open devices."
                ),
            ));
        }

        let prev = self.current;
        self.current = n;
        Ok(prev)
    }

    /// Add a new device, returning its 1-based index.
    ///
    /// The device is inserted into the first available slot (index >= 2),
    /// or appended at the end if no empty slots exist. The new device
    /// becomes the current device.
    pub fn add_device(&mut self, device: Box<dyn GraphicsDevice>) -> usize {
        // Look for a free slot starting at index 2
        for i in 2..self.devices.len() {
            if self.devices[i].is_none() {
                self.devices[i] = Some(device);
                self.current = i;
                return i;
            }
        }

        // No free slot — append
        let idx = self.devices.len();
        self.devices.push(Some(device));
        self.current = idx;
        idx
    }

    /// Close the device at index `n`.
    ///
    /// Calls `device.close()` and removes it from the slot. If the closed
    /// device was the current device, the manager picks the next open device
    /// (or reverts to 1 if none remain).
    ///
    /// Closing the null device (index 1) is an error.
    pub fn close_device(&mut self, n: usize) -> Result<(), RError> {
        if n == 1 {
            return Err(RError::new(
                RErrorKind::Argument,
                "cannot shut down device 1 (the null device)".to_string(),
            ));
        }

        if n == 0 || n >= self.devices.len() {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "dev.off({n}): no device at index {n}\n  \
                     Use dev.list() to see open devices."
                ),
            ));
        }

        let slot = &mut self.devices[n];
        match slot.take() {
            Some(mut device) => {
                device.close();
            }
            None => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "dev.off({n}): no device at index {n}\n  \
                         Use dev.list() to see open devices."
                    ),
                ));
            }
        }

        // If the closed device was current, find the next open device
        if self.current == n {
            self.current = self.find_next_device();
        }

        Ok(())
    }

    /// Close all open devices.
    ///
    /// Iterates through all slots (skipping the null device at index 1),
    /// calls `close()` on each, and resets the current device to 1.
    /// Errors during individual device closes are silently ignored (matching
    /// R's `graphics.off()` behavior).
    pub fn close_all(&mut self) {
        for i in 2..self.devices.len() {
            if let Some(mut device) = self.devices[i].take() {
                device.close();
            }
        }
        self.current = 1;
    }

    /// List all open devices as `(index, name)` pairs.
    ///
    /// Does NOT include the null device (index 1) in the list, matching
    /// R's `dev.list()` behavior which only shows real devices.
    pub fn list(&self) -> Vec<(usize, String)> {
        let mut result = Vec::new();
        for (i, slot) in self.devices.iter().enumerate() {
            if i < 2 {
                continue; // skip padding (0) and null device (1)
            }
            if let Some(device) = slot {
                result.push((i, device.name().to_string()));
            }
        }
        result
    }

    /// Execute a closure with mutable access to the current device.
    ///
    /// Returns an error if the current device is the null device (index 1)
    /// or if no device is open at the current index.
    pub fn with_current_mut<F, R>(&mut self, f: F) -> Result<R, RError>
    where
        F: FnOnce(&mut dyn GraphicsDevice) -> R,
    {
        if self.current <= 1 {
            return Err(RError::new(
                RErrorKind::Argument,
                "no active graphics device — use dev.new() to open one".to_string(),
            ));
        }

        let device = self.devices[self.current].as_mut().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                format!(
                    "device {} is no longer open — it may have been closed",
                    self.current
                ),
            )
        })?;

        Ok(f(device.as_mut()))
    }

    /// Find the next open device after a close, or return 1 (null device).
    fn find_next_device(&self) -> usize {
        // Prefer the highest-numbered open device (R's behavior)
        for i in (2..self.devices.len()).rev() {
            if self.devices[i].is_some() {
                return i;
            }
        }
        1 // null device
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::graphics::NullDevice;

    #[test]
    fn new_manager_starts_at_null_device() {
        let mgr = DeviceManager::new();
        assert_eq!(mgr.current(), 1);
    }

    #[test]
    fn add_device_returns_index_2_and_sets_current() {
        let mut mgr = DeviceManager::new();
        let idx = mgr.add_device(Box::new(NullDevice));
        assert_eq!(idx, 2);
        assert_eq!(mgr.current(), 2);
    }

    #[test]
    fn add_multiple_devices() {
        let mut mgr = DeviceManager::new();
        let idx1 = mgr.add_device(Box::new(NullDevice));
        let idx2 = mgr.add_device(Box::new(NullDevice));
        assert_eq!(idx1, 2);
        assert_eq!(idx2, 3);
        assert_eq!(mgr.current(), 3);
    }

    #[test]
    fn close_device_reverts_to_previous() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        mgr.add_device(Box::new(NullDevice));
        mgr.close_device(3).unwrap();
        // Should revert to device 2 (the remaining open device)
        assert_eq!(mgr.current(), 2);
    }

    #[test]
    fn close_last_device_reverts_to_null() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        mgr.close_device(2).unwrap();
        assert_eq!(mgr.current(), 1);
    }

    #[test]
    fn close_null_device_is_error() {
        let mut mgr = DeviceManager::new();
        let result = mgr.close_device(1);
        assert!(result.is_err());
    }

    #[test]
    fn close_nonexistent_device_is_error() {
        let mut mgr = DeviceManager::new();
        let result = mgr.close_device(99);
        assert!(result.is_err());
    }

    #[test]
    fn set_current_valid() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        mgr.add_device(Box::new(NullDevice));
        let prev = mgr.set_current(2).unwrap();
        assert_eq!(prev, 3); // was at device 3 after second add
        assert_eq!(mgr.current(), 2);
    }

    #[test]
    fn set_current_to_null_device() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        let prev = mgr.set_current(1).unwrap();
        assert_eq!(prev, 2);
        assert_eq!(mgr.current(), 1);
    }

    #[test]
    fn set_current_invalid_is_error() {
        let mut mgr = DeviceManager::new();
        let result = mgr.set_current(5);
        assert!(result.is_err());
    }

    #[test]
    fn list_returns_only_open_devices() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        mgr.add_device(Box::new(NullDevice));
        let devices = mgr.list();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].0, 2);
        assert_eq!(devices[0].1, "null device");
        assert_eq!(devices[1].0, 3);
    }

    #[test]
    fn list_excludes_closed_devices() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        mgr.add_device(Box::new(NullDevice));
        mgr.close_device(2).unwrap();
        let devices = mgr.list();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].0, 3);
    }

    #[test]
    fn close_all_reverts_to_null() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        mgr.add_device(Box::new(NullDevice));
        mgr.close_all();
        assert_eq!(mgr.current(), 1);
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn with_current_mut_on_null_device_is_error() {
        let mut mgr = DeviceManager::new();
        let result = mgr.with_current_mut(|_dev| ());
        assert!(result.is_err());
    }

    #[test]
    fn with_current_mut_on_real_device_works() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice));
        let name = mgr.with_current_mut(|dev| dev.name().to_string()).unwrap();
        assert_eq!(name, "null device");
    }

    #[test]
    fn slot_reuse_after_close() {
        let mut mgr = DeviceManager::new();
        mgr.add_device(Box::new(NullDevice)); // index 2
        mgr.add_device(Box::new(NullDevice)); // index 3
        mgr.close_device(2).unwrap();
        let reused = mgr.add_device(Box::new(NullDevice)); // should reuse index 2
        assert_eq!(reused, 2);
    }
}
