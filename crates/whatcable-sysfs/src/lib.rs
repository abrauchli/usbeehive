//! Linux sysfs backend for [WhatCable](https://github.com/Zetaphor/whatcable-linux).
//!
//! This crate enumerates USB / Type-C / USB Power Delivery state from the
//! kernel-published `/sys` virtual filesystem, returning the structured
//! types defined in [`whatcable-core`](https://docs.rs/whatcable-core).
//!
//! # Injectable root
//!
//! All enumeration goes through a [`Sysfs`] handle, which carries the root
//! path. Production code uses [`Sysfs::linux`] (root = `/sys`); tests and
//! fixture-based tools point [`Sysfs::with_root`] at a captured tree on
//! disk. Nothing reads `/sys` paths directly.
//!
//! ```no_run
//! use whatcable_sysfs::DeviceManager;
//!
//! let mut mgr = DeviceManager::new();
//! mgr.refresh();
//! for s in mgr.devices() {
//!     println!("{}: {}", s.headline, s.subtitle);
//! }
//! ```
//!
//! # Errors
//!
//! Enumeration is intentionally infallible: if a directory is missing or
//! an attribute can't be parsed, the affected entry is silently skipped.
//! That matches kernel sysfs semantics — files come and go as drivers
//! load. The single exception is [`Sysfs::try_with_root`], which validates
//! that the root path exists.
//!
//! # Re-exports
//!
//! For convenience, the entire public surface of [`whatcable-core`] is
//! re-exported under [`core`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod manager;
pub mod power;
pub mod sysfs;
pub mod typec;
pub mod usb;

pub use error::{Error, Result};
pub use manager::{build_summaries, DeviceManager, Snapshot};
pub use sysfs::Sysfs;
pub use typec::ucsi_controller;

/// Re-export of [`whatcable_core`] for convenience.
pub use whatcable_core as core;
