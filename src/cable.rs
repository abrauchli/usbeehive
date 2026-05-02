//! Decoded cable e-marker info from a `TypeCCable`.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::pd::{decode_cable_vdo, decode_id_header, CableCurrent, CableSpeed, ProductType};
use crate::typec::TypeCCable;
use crate::vendor;

#[derive(Debug, Clone, Default, Serialize)]
pub struct CableInfo {
    pub is_active: bool,
    pub is_passive: bool,
    pub cable_type: String,
    pub plug_type: String,

    pub speed: Option<CableSpeed>,
    pub current_rating: Option<CableCurrent>,
    pub max_watts: u32,

    pub vendor_id: u16,
    pub vendor_name: String,

    pub raw_attributes: BTreeMap<String, String>,
}

impl CableInfo {
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
        // ID header: ufp = ActiveCable (4)
        let id_hdr = 4u32 << 27;
        // CableVDO: speed=2 (Gen2), current=2 (5A), voltage=3 (50V)
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
}
