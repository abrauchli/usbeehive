//! Pure data types and decoders for USB, USB Type-C, and USB Power Delivery.
//!
//! This crate is the IO-free heart of [WhatCable](https://github.com/Zetaphor/whatcable-linux).
//! It contains:
//!
//! - **Type definitions** ([`UsbDevice`], [`TypeCPort`], [`PowerDeliveryPort`], …)
//!   that represent a snapshot of devices observed in `/sys`. They are
//!   [`serde::Serialize`]-able so consumers can ship them over a wire or to JSON.
//! - **USB Power Delivery 3.x VDO decoders** ([`pd::decode_id_header`],
//!   [`pd::decode_cable_vdo`]) — useful on their own for anything parsing PD
//!   Discover Identity responses.
//! - **Diagnostics** ([`ChargingDiagnostic`]) that explain why a USB-C port
//!   isn't charging at full speed.
//! - **Plain-English summaries** ([`DeviceSummary`]) suitable for direct display.
//!
//! There is no IO here. To enumerate real devices, use the companion
//! [`whatcable-sysfs`](https://docs.rs/whatcable-sysfs) crate, or build your
//! own backend (eg. for parsing captured fixtures or remote inventory data).
//!
//! # Example
//!
//! Decoding a Discover Identity ID Header VDO byte-for-byte:
//!
//! ```
//! use whatcable_core::pd::{decode_id_header, ProductType};
//!
//! // ID Header reporting an Active Cable in the UFP product-type field.
//! let raw: u32 = (4 << 27) | 0x05AC; // Apple VID
//! let hdr = decode_id_header(raw);
//! assert_eq!(hdr.vendor_id, 0x05AC);
//! assert_eq!(hdr.ufp_product_type, Some(ProductType::ActiveCable));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cable;
pub mod diagnostic;
pub mod pd;
pub mod power;
pub mod summary;
pub mod typec;
pub mod usb;
pub mod usbclass;
pub mod vendor;

pub use cable::CableInfo;
pub use diagnostic::{Bottleneck, ChargingDiagnostic};
pub use power::{PdoType, PowerDataObject, PowerDeliveryPort};
pub use summary::{Category, DeviceSummary, Status};
pub use typec::{TypeCCable, TypeCIdentity, TypeCPartner, TypeCPort, TypeCPowerSupply};
pub use usb::{UsbDevice, UsbInterface};
