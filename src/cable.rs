//! Decoded cable e-marker information.
//!
//! [`CableInfo::from_typec_cable`] is the bridge from the raw [`TypeCCable`]
//! sysfs snapshot to a strongly-typed cable view, applying the VDO decoders
//! from [`crate::pd`].

use std::collections::BTreeMap;

use serde::Serialize;

use crate::pd::{
    cable_vdo_reserved_bits_set, decode_cable_vdo, decode_id_header, CableCurrent, CableSpeed,
    ProductType,
};
use crate::typec::TypeCCable;
use crate::vendor;

/// Cable trust signals.
///
/// Three heuristics that, taken together, hint at a counterfeit or buggy
/// e-marker. None is conclusive — UI consumers should render these with a
/// hedged tone (e.g. *"this cable's identity looks unusual"*).
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct CableTrust {
    /// `true` when the cable's ID Header VDO reports a vendor ID of zero.
    /// USB-IF certified e-markers always carry a registered VID.
    pub zero_vid: bool,
    /// `true` when the vendor ID is non-zero but isn't in the bundled
    /// USB-IF vendor database (i.e. [`vendor::lookup`] fell back to a hex
    /// string).
    pub vid_unknown: bool,
    /// `true` when the raw Cable VDO sets bits that USB-PD R3.x defines
    /// as reserved (see [`cable_vdo_reserved_bits_set`]).
    pub reserved_bits_set: bool,
}

impl CableTrust {
    /// `true` when any of the three heuristics fire.
    pub fn any(&self) -> bool {
        self.zero_vid || self.vid_unknown || self.reserved_bits_set
    }
}

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

    /// Trust signals derived from the cable's e-marker. See [`CableTrust`].
    pub trust: CableTrust,

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
        info.trust.zero_vid = id.vendor_id == 0;
        info.trust.vid_unknown = id.vendor_id != 0 && vendor::is_hex_fallback(&info.vendor_name);
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
            info.trust.reserved_bits_set = cable_vdo_reserved_bits_set(fourth, active);
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
    fn trust_zero_vid_fires_on_blank_emarker() {
        let cable = TypeCCable {
            r#type: "passive".into(),
            plug_type: String::new(),
            identity: Some(id(vec![3u32 << 27, 0, 0, 1], 0)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert!(info.trust.zero_vid);
        assert!(!info.trust.vid_unknown);
    }

    #[test]
    fn trust_vid_unknown_fires_on_hex_fallback() {
        // 0xDEAD isn't in the bundled vendor DB — lookup falls back to a
        // hex string and vid_unknown fires.
        let cable = TypeCCable {
            r#type: "passive".into(),
            plug_type: String::new(),
            identity: Some(id(vec![3u32 << 27, 0, 0, 1], 0xDEAD)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert!(!info.trust.zero_vid);
        assert!(info.trust.vid_unknown);
    }

    #[test]
    fn trust_reserved_bits_fires_on_dirty_vdo() {
        let dirty = 1u32 | (1 << 5) | (1 << 3); // bit 3 is reserved
        let cable = TypeCCable {
            r#type: "passive".into(),
            plug_type: String::new(),
            identity: Some(id(vec![3u32 << 27, 0, 0, dirty], 0x05AC)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert!(info.trust.reserved_bits_set);
        assert!(!info.trust.vid_unknown); // Apple is in the vendor DB
    }

    #[test]
    fn trust_clean_cable_fires_nothing() {
        let clean = 2u32 | (2 << 5) | (3 << 9);
        let cable = TypeCCable {
            r#type: "passive".into(),
            plug_type: String::new(),
            identity: Some(id(vec![3u32 << 27, 0, 0, clean], 0x05AC)),
            raw_attributes: BTreeMap::new(),
        };
        let info = CableInfo::from_typec_cable(&cable);
        assert!(!info.trust.any());
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
