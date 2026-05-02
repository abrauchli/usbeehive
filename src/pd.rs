//! USB Power Delivery 3.x VDO bit-field decoders.
//!
//! Ported from the original WhatCable Swift implementation. See USB PD R3
//! specification, "Discover Identity" responses.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProductType {
    Undefined,
    Hub,
    Peripheral,
    PassiveCable,
    ActiveCable,
    /// Alternate Mode Adapter
    Ama,
    /// VCONN-Powered Device
    Vpd,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CableSpeed {
    Usb20,
    /// 5 Gbps
    Usb32Gen1,
    /// 10 Gbps
    Usb32Gen2,
    /// 20 Gbps single-lane / 40 Gbps dual-lane
    Usb4Gen3,
    /// 40 Gbps single-lane / 80 Gbps dual-lane
    Usb4Gen4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CableCurrent {
    UsbDefault,
    ThreeAmp,
    FiveAmp,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IdHeaderVdo {
    pub usb_comm_capable_as_host: bool,
    pub usb_comm_capable_as_device: bool,
    pub modal_operation: bool,
    pub ufp_product_type: Option<ProductType>,
    pub dfp_product_type: Option<ProductType>,
    pub vendor_id: u16,
}

impl Default for ProductType {
    fn default() -> Self {
        ProductType::Undefined
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CableVdo {
    pub speed: CableSpeed,
    pub current_rating: CableCurrent,
    pub vbus_through_cable: bool,
    pub max_vbus_volts: u32,
    pub is_active: bool,
    pub max_watts: u32,
}

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

pub fn cable_speed_label(s: CableSpeed) -> &'static str {
    match s {
        CableSpeed::Usb20 => "USB 2.0",
        CableSpeed::Usb32Gen1 => "USB 3.2 Gen 1 (5 Gbps)",
        CableSpeed::Usb32Gen2 => "USB 3.2 Gen 2 (10 Gbps)",
        CableSpeed::Usb4Gen3 => "USB4 Gen 3 (20/40 Gbps)",
        CableSpeed::Usb4Gen4 => "USB4 Gen 4 (40/80 Gbps)",
    }
}

pub fn cable_speed_max_gbps(s: CableSpeed) -> u32 {
    match s {
        CableSpeed::Usb20 => 0,
        CableSpeed::Usb32Gen1 => 5,
        CableSpeed::Usb32Gen2 => 10,
        CableSpeed::Usb4Gen3 => 40,
        CableSpeed::Usb4Gen4 => 80,
    }
}

pub fn cable_current_label(c: CableCurrent) -> &'static str {
    match c {
        CableCurrent::UsbDefault => "USB Default",
        CableCurrent::ThreeAmp => "3A",
        CableCurrent::FiveAmp => "5A",
    }
}

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
        // Bits 27..29 = 4 -> ActiveCable
        let h = decode_id_header(4 << 27);
        assert_eq!(h.ufp_product_type, Some(ProductType::ActiveCable));
    }

    #[test]
    fn id_header_flag_bits() {
        let h = decode_id_header((1u32 << 31) | (1 << 30) | (1 << 26));
        assert!(h.usb_comm_capable_as_host);
        assert!(h.usb_comm_capable_as_device);
        assert!(h.modal_operation);
    }

    #[test]
    fn cable_vdo_decodes_speed_current_voltage() {
        // speed=2 (Gen2), current=1 (3A), voltage=0 (20V)
        let v = decode_cable_vdo(2 | (1 << 5), false);
        assert_eq!(v.speed, CableSpeed::Usb32Gen2);
        assert_eq!(v.current_rating, CableCurrent::ThreeAmp);
        assert_eq!(v.max_vbus_volts, 20);
        assert_eq!(v.max_watts, 60);
        assert!(!v.is_active);
    }

    #[test]
    fn cable_vdo_5a_50v_marks_240w() {
        let v = decode_cable_vdo((2 << 5) | (3 << 9), true);
        assert_eq!(v.current_rating, CableCurrent::FiveAmp);
        assert_eq!(v.max_vbus_volts, 50);
        assert_eq!(v.max_watts, 250);
        assert!(v.is_active);
    }

    #[test]
    fn cable_vdo_unknown_speed_falls_to_usb20() {
        let v = decode_cable_vdo(7, false);
        assert_eq!(v.speed, CableSpeed::Usb20);
    }

    #[test]
    fn label_helpers() {
        assert_eq!(product_type_label(ProductType::Hub), "USB Hub");
        assert_eq!(cable_speed_label(CableSpeed::Usb4Gen4), "USB4 Gen 4 (40/80 Gbps)");
        assert_eq!(cable_speed_max_gbps(CableSpeed::Usb32Gen1), 5);
        assert_eq!(cable_current_label(CableCurrent::FiveAmp), "5A");
        assert!((cable_current_max_amps(CableCurrent::ThreeAmp) - 3.0).abs() < 1e-9);
    }
}
