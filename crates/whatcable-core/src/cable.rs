//! Decoded cable e-marker information.
//!
//! [`CableInfo::from_typec_cable`] is the bridge from the raw [`TypeCCable`]
//! sysfs snapshot to a strongly-typed cable view, applying the VDO decoders
//! from [`crate::pd`].

use std::collections::BTreeMap;

use serde::Serialize;

use crate::pd::{decode_cable_vdo, decode_id_header, CableCurrent, CableSpeed, ProductType};
use crate::typec::TypeCCable;
use crate::vendor;

/// Decoded view of a cable's e-marker.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CableInfo {
    /// `true` if the cable is electrically active.
    pub is_active: bool,
    /// `true` if the cable is passive (plain wires + e-marker).
    pub is_passive: bool,
    /// Raw `type` string from sysfs (`"active"` / `"passive"`).
    pub cable_type: String,
    /// Raw `plug_type` string.
    pub plug_type: String,

    /// Decoded link speed, when a Cable VDO is present.
    pub speed: Option<CableSpeed>,
    /// Decoded VBUS current rating.
    pub current_rating: Option<CableCurrent>,
    /// `current_rating × max_vbus_volts`, as a convenience.
    pub max_watts: u32,

    /// Cable vendor ID (low 16 bits of the ID Header VDO).
    pub vendor_id: u16,
    /// Friendly vendor name (or hex fallback) for `vendor_id`.
    pub vendor_name: String,

    /// Raw sysfs attributes captured for `--raw` rendering.
    pub raw_attributes: BTreeMap<String, String>,
}

impl CableInfo {
    /// Decode a [`TypeCCable`] snapshot into a [`CableInfo`].
    ///
    /// When the partner advertises an ID Header reporting an active cable,
    /// [`Self::is_active`] is set even if the kernel `type` attribute said
    /// `"passive"` — VDO data is the more authoritative source.
    pub fn from_typec_cable(cable: &TypeCCable) -> CableInfo {
        let mut info = CableInfo {
            cable_type: cable.r#type.clone(),
            plug_type: cable.plug_type.clone(),
            is_active: cable.r#type == "active",
            is_passive: cable.r#type == "passive",
            raw_attributes: cable.raw_attributes.clone(),
            ..Default::default()
        };
        let Some(id) = &cable.identity else {
            return info;
        };
        info.vendor_id = id.vendor_id;
        info.vendor_name = vendor::lookup(id.vendor_id);
        let Some(&first) = id.vdos.first() else {
            return info;
        };
        let hdr = decode_id_header(first);
        let active = hdr.ufp_product_type == Some(ProductType::ActiveCable);
        if let Some(&fourth) = id.vdos.get(3) {
            let v = decode_cable_vdo(fourth, active);
            info.speed = Some(v.speed);
            info.current_rating = Some(v.current_rating);
            info.max_watts = v.max_watts;
            info.is_active = v.is_active;
            info.is_passive = !v.is_active;
        }
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typec::TypeCIdentity;

    fn id(vdos: Vec<u32>, vid: u16) -> TypeCIdentity {
        TypeCIdentity {
            vendor_id: vid,
            product_id: 0,
            vdos,
        }
    }

    #[test]
    fn passive_string_only() {
        let c = TypeCCable {
            r#type: "passive".into(),
            plug_type: "type-c".into(),
            identity: None,
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&c);
        assert!(info.is_passive);
        assert!(info.speed.is_none());
        assert_eq!(info.max_watts, 0);
    }

    #[test]
    fn cable_vdo_drives_active_flag() {
        let id_hdr = 4u32 << 27;
        let cable_vdo = 2u32 | (2 << 5) | (3 << 9);
        let cable = TypeCCable {
            r#type: "passive".into(), // overridden by VDO active flag
            plug_type: String::new(),
            identity: Some(id(vec![id_hdr, 0, 0, cable_vdo], 0x05AC)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert_eq!(info.vendor_name, "Apple");
        assert_eq!(info.speed, Some(CableSpeed::Usb32Gen2));
        assert_eq!(info.current_rating, Some(CableCurrent::FiveAmp));
        assert_eq!(info.max_watts, 250);
        assert!(info.is_active);
        assert!(!info.is_passive);
    }

    #[test]
    fn passive_with_vdo_keeps_passive_flag() {
        let id_hdr = 3u32 << 27; // PassiveCable
        let cable_vdo = 1u32 | (1 << 5); // Gen1, 3A
        let cable = TypeCCable {
            r#type: "passive".into(),
            plug_type: String::new(),
            identity: Some(id(vec![id_hdr, 0, 0, cable_vdo], 0x0BDA)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert_eq!(info.speed, Some(CableSpeed::Usb32Gen1));
        assert_eq!(info.current_rating, Some(CableCurrent::ThreeAmp));
        assert!(info.is_passive);
        assert!(!info.is_active);
        assert_eq!(info.vendor_name, "Realtek");
    }

    #[test]
    fn missing_cable_vdo_keeps_only_vendor() {
        let cable = TypeCCable {
            r#type: "passive".into(),
            plug_type: String::new(),
            identity: Some(id(vec![1u32 << 31], 0x05AC)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert_eq!(info.vendor_name, "Apple");
        assert!(info.speed.is_none());
        assert_eq!(info.max_watts, 0);
    }
}
