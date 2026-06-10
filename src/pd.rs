//! USB Power Delivery 3.x VDO decoders.
//!
//! USB-PD partners exchange capability information through 32-bit *Vendor
//! Defined Objects* (VDOs). This module decodes the bit-fields defined by
//! the USB Power Delivery R3.x specification's "Discover Identity" command:
//!
//! - [`decode_id_header`] — parses the **ID Header VDO** (always the first
//!   VDO of a Discover Identity response).
//! - [`decode_cable_vdo`] — parses a **Cable VDO** (passive cables: 4th VDO;
//!   active cables: 4th and 5th VDOs).
//!
//! Decoders are pure functions: hand them a `u32` and they return strongly
//! typed enums. They never read sysfs, never panic, and never allocate.
//!
//! # Example
//!
//! ```
//! use usbeehive::pd::{decode_cable_vdo, CableSpeed, CableCurrent};
//!
//! // CableVDO bits: speed=2 (USB 3.2 Gen 2), current=2 (5A), max VBUS=3 (50V)
//! let raw = 2u32 | (2 << 5) | (3 << 9);
//! let v = decode_cable_vdo(raw, /* is_active = */ false);
//! assert_eq!(v.speed, CableSpeed::Usb32Gen2);
//! assert_eq!(v.current_rating, CableCurrent::FiveAmp);
//! assert_eq!(v.max_vbus_volts, 50);
//! assert_eq!(v.max_watts, 250);
//! ```

use serde::Serialize;

/// The kind of USB-PD product reported in a `Discover Identity` ID Header.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProductType {
    /// Field absent or reserved.
    #[default]
    Undefined,
    /// USB hub.
    Hub,
    /// USB peripheral (storage, HID, audio, …).
    Peripheral,
    /// Passive USB-C cable assembly.
    PassiveCable,
    /// Active USB-C cable assembly (powered re-driver / re-timer).
    ActiveCable,
    /// Alternate Mode Adapter (AMA).
    Ama,
    /// VCONN-Powered Device (VPD).
    Vpd,
    /// Reserved or vendor-specific value not interpreted.
    Other,
}

/// USB data-rate capability advertised in a Cable VDO.
///
/// Invariant: variants are declared in ascending speed order — the
/// derived `Ord` relies on it (`Usb20 < Usb32Gen1 < … < Usb4Gen4`), so
/// callers can compare a cable's rating against a device's capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum CableSpeed {
    /// USB 2.0 (480 Mbps), or no SuperSpeed support advertised.
    Usb20,
    /// USB 3.2 Gen 1 — 5 Gbps single lane.
    Usb32Gen1,
    /// USB 3.2 Gen 2 — 10 Gbps single lane.
    Usb32Gen2,
    /// USB4 Gen 3 — 20 Gbps single lane / 40 Gbps dual lane.
    Usb4Gen3,
    /// USB4 Gen 4 — 40 Gbps single lane / 80 Gbps dual lane.
    Usb4Gen4,
}

/// VBUS current rating advertised by a cable's e-marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CableCurrent {
    /// USB Default — only nominal 5V @ 0.9A guaranteed.
    UsbDefault,
    /// 3 A rating (60W at 20V VBUS).
    ThreeAmp,
    /// 5 A rating (100W at 20V, 240W at 48V).
    FiveAmp,
}

/// Decoded fields of an ID Header VDO.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdHeaderVdo {
    /// USB Communications Capable as Host (DFP).
    pub usb_comm_capable_as_host: bool,
    /// USB Communications Capable as Device (UFP).
    pub usb_comm_capable_as_device: bool,
    /// Modal Operation Supported (i.e. supports Alternate Modes).
    pub modal_operation: bool,
    /// Product type advertised in the UFP role, if recognized.
    pub ufp_product_type: Option<ProductType>,
    /// Product type advertised in the DFP role, if recognized.
    pub dfp_product_type: Option<ProductType>,
    /// Cable / device vendor ID (low 16 bits of the VDO).
    pub vendor_id: u16,
}

/// Decoded fields of a Cable VDO.
#[derive(Debug, Clone, Copy)]
pub struct CableVdo {
    /// Maximum link speed supported by the cable.
    pub speed: CableSpeed,
    /// Maximum VBUS current rating.
    pub current_rating: CableCurrent,
    /// `true` when the cable carries VBUS through to the partner.
    pub vbus_through_cable: bool,
    /// Maximum VBUS voltage supported, in volts (20, 30, 40, or 50).
    pub max_vbus_volts: u32,
    /// `true` when the cable is electrically active (re-driver / re-timer).
    pub is_active: bool,
    /// Convenience product of `current_rating` × `max_vbus_volts`, in watts.
    pub max_watts: u32,
}

/// Human-readable label for a [`ProductType`].
pub fn product_type_label(t: ProductType) -> &'static str {
    match t {
        ProductType::Hub => "USB Hub",
        ProductType::Peripheral => "USB Peripheral",
        ProductType::PassiveCable => "Passive Cable",
        ProductType::ActiveCable => "Active Cable",
        ProductType::Ama => "Alternate Mode Adapter",
        ProductType::Vpd => "VCONN-Powered Device",
        ProductType::Other => "Other",
        ProductType::Undefined => "Unknown",
    }
}

/// Human-readable label for a [`CableSpeed`].
pub fn cable_speed_label(s: CableSpeed) -> &'static str {
    match s {
        CableSpeed::Usb20 => "USB 2.0",
        CableSpeed::Usb32Gen1 => "USB 3.2 Gen 1 (5 Gbps)",
        CableSpeed::Usb32Gen2 => "USB 3.2 Gen 2 (10 Gbps)",
        CableSpeed::Usb4Gen3 => "USB4 Gen 3 (20/40 Gbps)",
        CableSpeed::Usb4Gen4 => "USB4 Gen 4 (40/80 Gbps)",
    }
}

/// Headline single-lane Gbps figure for a [`CableSpeed`].
///
/// Returns 0 for [`CableSpeed::Usb20`].
pub fn cable_speed_max_gbps(s: CableSpeed) -> u32 {
    match s {
        CableSpeed::Usb20 => 0,
        CableSpeed::Usb32Gen1 => 5,
        CableSpeed::Usb32Gen2 => 10,
        CableSpeed::Usb4Gen3 => 40,
        CableSpeed::Usb4Gen4 => 80,
    }
}

/// Human-readable label for a [`CableCurrent`].
pub fn cable_current_label(c: CableCurrent) -> &'static str {
    match c {
        CableCurrent::UsbDefault => "USB Default",
        CableCurrent::ThreeAmp => "3A",
        CableCurrent::FiveAmp => "5A",
    }
}

/// Numeric ampere figure for a [`CableCurrent`].
pub fn cable_current_max_amps(c: CableCurrent) -> f64 {
    match c {
        CableCurrent::UsbDefault => 0.9,
        CableCurrent::ThreeAmp => 3.0,
        CableCurrent::FiveAmp => 5.0,
    }
}

fn ufp_from_bits(bits: u32) -> Option<ProductType> {
    match bits {
        1 => Some(ProductType::Hub),
        2 => Some(ProductType::Peripheral),
        3 => Some(ProductType::PassiveCable),
        4 => Some(ProductType::ActiveCable),
        5 => Some(ProductType::Ama),
        6 => Some(ProductType::Vpd),
        _ => None,
    }
}

fn dfp_from_bits(bits: u32) -> Option<ProductType> {
    match bits {
        1 => Some(ProductType::Hub),
        2 => Some(ProductType::Peripheral),
        _ => None,
    }
}

/// Decode the ID Header VDO into typed fields.
///
/// See USB-PD R3.x § 6.4.4.2.1.
pub fn decode_id_header(vdo: u32) -> IdHeaderVdo {
    IdHeaderVdo {
        usb_comm_capable_as_host: (vdo >> 31) & 1 != 0,
        usb_comm_capable_as_device: (vdo >> 30) & 1 != 0,
        modal_operation: (vdo >> 26) & 1 != 0,
        ufp_product_type: ufp_from_bits((vdo >> 27) & 0x7),
        dfp_product_type: dfp_from_bits((vdo >> 23) & 0x7),
        vendor_id: (vdo & 0xFFFF) as u16,
    }
}

/// Decode the *USB Highest Speed* field of a UFP VDO1.
///
/// `vdo1` is the fourth VDO of a Discover Identity response
/// (`product_type_vdo1`) when the ID Header's `ufp_product_type` is
/// [`ProductType::Hub`] or [`ProductType::Peripheral`] — callers gate on
/// that; this function only decodes the bits.
///
/// Returns `None` when `vdo1` is zero or its UFP VDO version bits
/// `[31:29]` are zero — PD 2.0 identities carry no UFP VDO, and the
/// sysfs reader pads the missing slot with zero. Bits `[2:0]` map
/// 0 → USB 2.0, 1 → Gen 1, 2 → Gen 2, 3 → USB4 Gen 3, 4 → USB4 Gen 4;
/// other values are reserved → `None`.
///
/// See USB-PD R3.x § 6.4.4.3.1.4 (UFP VDO).
pub fn decode_ufp_vdo_highest_speed(vdo1: u32) -> Option<CableSpeed> {
    if vdo1 == 0 || (vdo1 >> 29) & 0x7 == 0 {
        return None;
    }
    match vdo1 & 0x7 {
        0 => Some(CableSpeed::Usb20),
        1 => Some(CableSpeed::Usb32Gen1),
        2 => Some(CableSpeed::Usb32Gen2),
        3 => Some(CableSpeed::Usb4Gen3),
        4 => Some(CableSpeed::Usb4Gen4),
        _ => None,
    }
}

/// `true` when the raw Cable VDO sets bits that USB-PD R3.x defines as
/// reserved — a soft trust signal that the e-marker either is buggy or
/// reuses a counterfeit reference design.
///
/// Conservative mask: only the bits that are reserved across the entire
/// PD 3.x lifetime of the cable VDO layout. Specifically:
///
/// - Bit 3 — reserved in both passive and active cable VDOs.
/// - Bits 7..8 — reserved in passive cable VDOs (used for SBU support in
///   active cables; only checked when `is_active` is `false`).
///
/// Caller passes the same `is_active` flag used with [`decode_cable_vdo`].
pub fn cable_vdo_reserved_bits_set(vdo: u32, is_active: bool) -> bool {
    let mask: u32 = if is_active { 0x0000_0008 } else { 0x0000_0188 };
    vdo & mask != 0
}

/// Decode a Cable VDO.
///
/// `is_active` should be set from the partner's reported product type — the
/// resulting [`CableVdo::is_active`] field will be the same value, threaded
/// through for convenience.
///
/// See USB-PD R3.x § 6.4.4.3.1 (Passive Cable VDO) and § 6.4.4.3.1.1
/// (Active Cable VDO).
pub fn decode_cable_vdo(vdo: u32, is_active: bool) -> CableVdo {
    let speed = match vdo & 0x7 {
        1 => CableSpeed::Usb32Gen1,
        2 => CableSpeed::Usb32Gen2,
        3 => CableSpeed::Usb4Gen3,
        4 => CableSpeed::Usb4Gen4,
        _ => CableSpeed::Usb20,
    };
    let current_rating = match (vdo >> 5) & 0x3 {
        1 => CableCurrent::ThreeAmp,
        2 => CableCurrent::FiveAmp,
        _ => CableCurrent::UsbDefault,
    };
    let max_vbus_volts = match (vdo >> 9) & 0x3 {
        1 => 30,
        2 => 40,
        3 => 50,
        _ => 20,
    };
    let amps = cable_current_max_amps(current_rating);
    let max_watts = (max_vbus_volts as f64 * amps) as u32;
    CableVdo {
        speed,
        current_rating,
        vbus_through_cable: (vdo >> 4) & 1 != 0,
        max_vbus_volts,
        is_active,
        max_watts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_header_vendor_low16() {
        let h = decode_id_header(0x12345678);
        assert_eq!(h.vendor_id, 0x5678);
    }

    #[test]
    fn id_header_active_cable_ufp() {
        let h = decode_id_header(4 << 27);
        assert_eq!(h.ufp_product_type, Some(ProductType::ActiveCable));
    }

    #[test]
    fn id_header_dfp_field() {
        let h = decode_id_header(2 << 23);
        assert_eq!(h.dfp_product_type, Some(ProductType::Peripheral));
    }

    #[test]
    fn id_header_flag_bits() {
        let h = decode_id_header((1u32 << 31) | (1 << 30) | (1 << 26));
        assert!(h.usb_comm_capable_as_host);
        assert!(h.usb_comm_capable_as_device);
        assert!(h.modal_operation);
    }

    #[test]
    fn id_header_unknown_product_type_is_none() {
        // bits 27..29 = 7 → reserved
        let h = decode_id_header(7 << 27);
        assert_eq!(h.ufp_product_type, None);
    }

    #[test]
    fn cable_vdo_decodes_speed_current_voltage() {
        let v = decode_cable_vdo(2 | (1 << 5), false);
        assert_eq!(v.speed, CableSpeed::Usb32Gen2);
        assert_eq!(v.current_rating, CableCurrent::ThreeAmp);
        assert_eq!(v.max_vbus_volts, 20);
        assert_eq!(v.max_watts, 60);
        assert!(!v.is_active);
    }

    #[test]
    fn cable_vdo_5a_50v_marks_250w() {
        let v = decode_cable_vdo((2 << 5) | (3 << 9), true);
        assert_eq!(v.current_rating, CableCurrent::FiveAmp);
        assert_eq!(v.max_vbus_volts, 50);
        assert_eq!(v.max_watts, 250);
        assert!(v.is_active);
    }

    #[test]
    fn reserved_bits_passive_detect() {
        // Passive cable: bit 3 is reserved.
        assert!(cable_vdo_reserved_bits_set(0b1000, false));
        // Bit 7 reserved in passive.
        assert!(cable_vdo_reserved_bits_set(1 << 7, false));
        // A well-formed passive cable VDO (Gen2, 5A, 50V) sets none of them.
        let clean = 2u32 | (2 << 5) | (3 << 9);
        assert!(!cable_vdo_reserved_bits_set(clean, false));
    }

    #[test]
    fn reserved_bits_active_only_bit_3() {
        // Bit 3 still reserved in active.
        assert!(cable_vdo_reserved_bits_set(0b1000, true));
        // Bit 7 is *not* reserved in active (SBU). Must not fire.
        assert!(!cable_vdo_reserved_bits_set(1 << 7, true));
    }

    #[test]
    fn cable_vdo_unknown_speed_falls_to_usb20() {
        let v = decode_cable_vdo(7, false);
        assert_eq!(v.speed, CableSpeed::Usb20);
    }

    #[test]
    fn cable_vdo_vbus_through_bit() {
        let v = decode_cable_vdo(1 << 4, false);
        assert!(v.vbus_through_cable);
        let v = decode_cable_vdo(0, false);
        assert!(!v.vbus_through_cable);
    }

    #[test]
    fn ufp_vdo_highest_speed_decodes_known_values() {
        // Version bits [31:29] non-zero (PD 3.x UFP VDO version 1.3).
        let v = 0b011u32 << 29;
        assert_eq!(decode_ufp_vdo_highest_speed(v), Some(CableSpeed::Usb20));
        assert_eq!(
            decode_ufp_vdo_highest_speed(v | 2),
            Some(CableSpeed::Usb32Gen2)
        );
        assert_eq!(
            decode_ufp_vdo_highest_speed(v | 4),
            Some(CableSpeed::Usb4Gen4)
        );
    }

    #[test]
    fn ufp_vdo_highest_speed_rejects_zero_and_version_0() {
        // Zero VDO — PD 2.0 identity padded by the sysfs reader.
        assert_eq!(decode_ufp_vdo_highest_speed(0), None);
        // Non-zero payload but version bits [31:29] zero — not a UFP VDO.
        assert_eq!(decode_ufp_vdo_highest_speed(2), None);
    }

    #[test]
    fn ufp_vdo_highest_speed_rejects_reserved_speed() {
        let v = (0b011u32 << 29) | 7;
        assert_eq!(decode_ufp_vdo_highest_speed(v), None);
    }

    #[test]
    fn cable_speed_ord_follows_declaration_order() {
        // The Ord derive backs the cable-vs-device comparison; lock the
        // ascending declaration order.
        assert!(CableSpeed::Usb20 < CableSpeed::Usb32Gen2);
        assert!(CableSpeed::Usb32Gen2 < CableSpeed::Usb4Gen4);
        assert!(CableSpeed::Usb32Gen1 < CableSpeed::Usb32Gen2);
        assert!(CableSpeed::Usb4Gen3 < CableSpeed::Usb4Gen4);
    }

    #[test]
    fn label_helpers() {
        assert_eq!(product_type_label(ProductType::Hub), "USB Hub");
        assert_eq!(
            cable_speed_label(CableSpeed::Usb4Gen4),
            "USB4 Gen 4 (40/80 Gbps)"
        );
        assert_eq!(cable_speed_max_gbps(CableSpeed::Usb32Gen1), 5);
        assert_eq!(cable_speed_max_gbps(CableSpeed::Usb20), 0);
        assert_eq!(cable_current_label(CableCurrent::FiveAmp), "5A");
        assert!((cable_current_max_amps(CableCurrent::ThreeAmp) - 3.0).abs() < 1e-9);
    }
}
