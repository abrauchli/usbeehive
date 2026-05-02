//! WhatCable-Linux core library.
//!
//! Rust port of WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable),
//! based on the C++ Linux port at https://github.com/Zetaphor/whatcable-linux.

pub mod cable;
pub mod diagnostic;
pub mod manager;
#[cfg(feature = "watch")]
pub mod monitor;
pub mod output;
pub mod pd;
pub mod power;
pub mod summary;
pub mod sysfs;
pub mod typec;
pub mod usb;
pub mod usbclass;
pub mod vendor;

pub use cable::CableInfo;
pub use diagnostic::{Bottleneck, ChargingDiagnostic};
pub use manager::DeviceManager;
pub use pd::{
    decode_cable_vdo, decode_id_header, cable_current_label, cable_current_max_amps,
    cable_speed_label, cable_speed_max_gbps, product_type_label, CableCurrent, CableSpeed,
    CableVdo, IdHeaderVdo, ProductType,
};
pub use power::{PdoType, PowerDataObject, PowerDeliveryPort};
pub use summary::{Category, DeviceSummary, Status};
pub use typec::{TypeCCable, TypeCIdentity, TypeCPartner, TypeCPort, TypeCPowerSupply};
pub use usb::{UsbDevice, UsbInterface};
