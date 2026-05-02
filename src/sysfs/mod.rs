//! Linux sysfs backend.
//!
//! Walks `/sys/bus/usb/devices`, `/sys/class/typec`, and
//! `/sys/class/usb_power_delivery`, returning the structured types from
//! the rest of the crate. Construction goes through a [`reader::Sysfs`] handle
//! that carries the root path, so tests and fixture-based tools can
//! point enumeration at a captured tree on disk.
//!
//! ```no_run
//! use whatcable::DeviceManager;
//!
//! let mut mgr = DeviceManager::new();
//! mgr.refresh();
//! println!("{} devices", mgr.devices().len());
//! ```

pub mod error;
pub mod manager;
pub mod power;
pub mod reader;
pub mod typec;
pub mod usb;
