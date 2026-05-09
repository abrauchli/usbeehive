//! WhatCable: USB / USB-C / USB Power Delivery diagnostics for Linux.
//!
//! This crate ships the [`whatcable`](https://crates.io/crates/whatcable) CLI
//! plus a library API. The library has three layers, each toggled by a Cargo
//! feature so consumers pull in only what they need:
//!
//! | Feature | Default | What it adds |
//! |---|---|---|
//! | (none) | always on | Pure data types ([`UsbDevice`], [`TypeCPort`], [`PowerDeliveryPort`], …), USB-PD VDO decoders ([`pd::decode_id_header`], [`pd::decode_cable_vdo`]), [`ChargingDiagnostic`], [`DeviceSummary`]. No IO. |
//! | `sysfs` | yes | [`Sysfs`] handle and [`DeviceManager`] — Linux `/sys` enumeration with an injectable root path. |
//! | `watch` | yes | libudev hotplug monitor: [`watch::Watcher`] + [`watch::run_loop`]. |
//! | `cli` | yes | The `whatcable` binary (clap + JSON / text rendering). |
//!
//! Library-only consumers can drop the binary deps:
//!
//! ```toml
//! [dependencies]
//! whatcable = { version = "0.3", default-features = false }
//! # for the sysfs backend without the bin / hotplug:
//! whatcable = { version = "0.3", default-features = false, features = ["sysfs"] }
//! ```
//!
//! # Example — decode a Cable VDO
//!
//! ```
//! use whatcable::pd::{decode_cable_vdo, CableSpeed, CableCurrent};
//!
//! let v = decode_cable_vdo(2 | (2 << 5) | (3 << 9), false);
//! assert_eq!(v.speed, CableSpeed::Usb32Gen2);
//! assert_eq!(v.current_rating, CableCurrent::FiveAmp);
//! assert_eq!(v.max_watts, 250);
//! ```
//!
//! # Example — enumerate `/sys`
//!
//! ```no_run
//! # #[cfg(feature = "sysfs")] {
//! use whatcable::DeviceManager;
//!
//! let mut mgr = DeviceManager::new();
//! mgr.refresh();
//! for s in mgr.devices() {
//!     println!("{}: {}", s.headline, s.subtitle);
//! }
//! # }
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod cable;
pub mod diagnostic;
pub mod pd;
pub mod power;
pub mod summary;
pub mod typec;
pub mod usb;
pub mod usbclass;
pub mod vendor;

#[cfg(feature = "sysfs")]
pub mod sysfs;

#[cfg(feature = "watch")]
pub mod watch;

pub use cable::CableInfo;
pub use diagnostic::{Bottleneck, ChargingDiagnostic};
pub use power::{PdoType, PowerDataObject, PowerDeliveryPort};
pub use summary::{Category, DeviceSummary, Status};
pub use typec::{TypeCCable, TypeCIdentity, TypeCPartner, TypeCPort, TypeCPowerSupply};
pub use usb::{LinkSpeed, UsbDevice, UsbInterface};

#[cfg(feature = "sysfs")]
pub use sysfs::{
    error::{Error, Result},
    manager::{build_summaries, DeviceManager, Snapshot, SnapshotDiff},
    reader::Sysfs,
    typec::ucsi_controller,
};

#[cfg(feature = "dbus")]
pub mod dbus;
