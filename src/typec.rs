//! USB Type-C port data model.
//!
//! Plain-data types corresponding to entries under `/sys/class/typec/`.
//! Enumeration lives in `usbeehive-sysfs`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

/// Decoded `Discover Identity` response from a partner or cable.
///
/// `vdos` holds every VDO file present under the kernel's `identity/`
/// directory in the order kernels publish them (`id_header`, `cert_stat`,
/// `product`, then `product_type_vdo1..3` if present), which is the same
/// order they appear in the USB-PD wire format.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct TypeCIdentity {
    /// Vendor ID extracted from the ID Header VDO.
    pub vendor_id: u16,
    /// Product ID extracted from the Product VDO.
    pub product_id: u16,
    /// Raw VDO values, in spec order.
    pub vdos: Vec<u32>,
}

/// One alternate-mode entry advertised by a Type-C partner or cable.
///
/// Mirrors a `/sys/class/typec/<port>-{partner,cable}.<n>/` directory.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct TypeCAltMode {
    /// Standard or Vendor ID — e.g. `0xFF01` for VESA DisplayPort,
    /// `0x8087` for Intel Thunderbolt.
    pub svid: u16,
    /// Mode index within the SVID (the kernel exposes one row per mode).
    pub mode: u32,
    /// `true` when the altmode has been entered (current contract).
    pub active: bool,
}

/// Snapshot of a Type-C **partner** device attached to a port.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct TypeCPartner {
    /// Kernel `type` attribute (e.g. `"upstream"`, `"downstream"`).
    pub r#type: String,
    /// Decoded Discover Identity, if the partner advertises one.
    pub identity: Option<TypeCIdentity>,
    /// Altmodes advertised by the partner (DisplayPort, Thunderbolt, …).
    pub altmodes: Vec<TypeCAltMode>,
    /// Every regular file under the partner sysfs directory.
    pub raw_attributes: BTreeMap<String, String>,
}

/// Snapshot of a Type-C **cable** plug attached to a port.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct TypeCCable {
    /// `type` attribute — `"passive"` or `"active"`.
    pub r#type: String,
    /// `plug_type` attribute (`"type-c"`, `"type-a"`, …).
    pub plug_type: String,
    /// Decoded Discover Identity, if the cable advertises an e-marker.
    pub identity: Option<TypeCIdentity>,
    /// Every regular file under the cable sysfs directory.
    pub raw_attributes: BTreeMap<String, String>,
}

/// UCSI power-supply view of an attached Type-C source.
///
/// Mirrors a `/sys/class/power_supply/ucsi-source-psy-USBC*` directory.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct TypeCPowerSupply {
    /// Sysfs path of the power-supply directory.
    pub sysfs_path: PathBuf,
    /// Power-supply name (basename of `sysfs_path`).
    pub name: String,
    /// `online` attribute.
    pub online: bool,
    /// `voltage_now` in microvolts.
    pub voltage_now_uv: Option<i64>,
    /// `current_now` in microamps.
    pub current_now_ua: Option<i64>,
    /// `current_max` in microamps.
    pub current_max_ua: Option<i64>,
    /// `voltage_min` in microvolts.
    pub voltage_min_uv: Option<i64>,
    /// `voltage_max` in microvolts.
    pub voltage_max_uv: Option<i64>,
    /// `charge_type` attribute.
    pub charge_type: String,
    /// `usb_type` attribute.
    pub usb_type: String,
    /// All regular files under the power-supply directory.
    pub raw_attributes: BTreeMap<String, String>,
}

impl TypeCPowerSupply {
    /// Live negotiated wattage in **mW**, computed as
    /// `voltage_now × current_now`. Uses `i128` internally so EPR
    /// voltages (up to 48V) don't overflow when multiplied by current.
    ///
    /// Returns `None` when either reading is missing or non-positive —
    /// callers should treat that as "no live wattage available right now",
    /// not as an error.
    pub fn negotiated_power_mw(&self) -> Option<i64> {
        let v = self.voltage_now_uv?;
        let i = self.current_now_ua?;
        if v <= 0 || i <= 0 {
            return None;
        }
        Some(((v as i128 * i as i128) / 1_000_000_000) as i64)
    }
}

/// Snapshot of one entry under `/sys/class/typec/`.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct TypeCPort {
    /// Absolute sysfs path of the port directory.
    pub sysfs_path: PathBuf,
    /// Kernel-assigned name (e.g. `"port0"`).
    pub port_name: String,
    /// Numeric port index parsed from `port_name`, or `-1` if unparseable.
    pub port_number: i32,

    /// Raw `data_role` attribute (e.g. `"host [device]"`).
    pub data_role: String,
    /// Raw `power_role` attribute (e.g. `"[source] sink"`).
    pub power_role: String,
    /// `port_type` (DRP / Source-only / …).
    pub port_type: String,
    /// `power_operation_mode`.
    pub power_op_mode: String,
    /// Plug `orientation` (`normal` / `reverse` / `unknown`).
    pub orientation: String,
    /// `usb_power_delivery_revision`.
    pub pd_revision: String,
    /// `usb_typec_revision`.
    pub usb_typec_rev: String,
    /// Companion UCSI power-supply, if one was located.
    pub power_supply: Option<TypeCPowerSupply>,

    /// Attached partner, if connected.
    pub partner: Option<TypeCPartner>,
    /// Attached cable, if any.
    pub cable: Option<TypeCCable>,

    /// All regular files under the port sysfs directory.
    pub raw_attributes: BTreeMap<String, String>,
}

impl TypeCPort {
    /// `true` when either a partner or a cable is currently attached.
    pub fn is_connected(&self) -> bool {
        self.partner.is_some() || self.cable.is_some()
    }

    /// `data_role` reduced to the bracketed active value (e.g. `"device"`
    /// from `"host [device]"`).
    pub fn current_data_role(&self) -> String {
        bracketed(&self.data_role)
    }

    /// `power_role` reduced to the bracketed active value.
    pub fn current_power_role(&self) -> String {
        bracketed(&self.power_role)
    }
}

/// Extract the value inside `[...]` from a sysfs choice list. Falls back to
/// the raw string when no brackets are present.
pub fn bracketed(s: &str) -> String {
    if let Some(start) = s.find('[') {
        if let Some(end) = s[start + 1..].find(']') {
            return s[start + 1..start + 1 + end].to_string();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracketed_extracts_active_role() {
        assert_eq!(bracketed("host [device]"), "device");
        assert_eq!(bracketed("[source] sink"), "source");
        assert_eq!(bracketed("plain"), "plain");
        assert_eq!(bracketed(""), "");
    }

    #[test]
    fn is_connected_logic() {
        let mut p = TypeCPort::default();
        assert!(!p.is_connected());
        p.cable = Some(TypeCCable::default());
        assert!(p.is_connected());
        p.cable = None;
        p.partner = Some(TypeCPartner::default());
        assert!(p.is_connected());
    }

    #[test]
    fn negotiated_power_mw_basic() {
        let psy = TypeCPowerSupply {
            voltage_now_uv: Some(5_000_000),
            current_now_ua: Some(3_000_000),
            ..Default::default()
        };
        assert_eq!(psy.negotiated_power_mw(), Some(15_000));
    }

    #[test]
    fn negotiated_power_mw_epr_no_overflow() {
        // 48V (EPR) × 5A = 240W = 240_000 mW. Naive i32 multiply overflows
        // 48_000_000 µV × 5_000_000 µA — make sure i128 path is reached.
        let psy = TypeCPowerSupply {
            voltage_now_uv: Some(48_000_000),
            current_now_ua: Some(5_000_000),
            ..Default::default()
        };
        assert_eq!(psy.negotiated_power_mw(), Some(240_000));
    }

    #[test]
    fn negotiated_power_mw_returns_none_on_missing() {
        let psy = TypeCPowerSupply::default();
        assert!(psy.negotiated_power_mw().is_none());

        let only_voltage = TypeCPowerSupply {
            voltage_now_uv: Some(5_000_000),
            ..Default::default()
        };
        assert!(only_voltage.negotiated_power_mw().is_none());
    }

    #[test]
    fn negotiated_power_mw_rejects_zero_or_negative() {
        let zero = TypeCPowerSupply {
            voltage_now_uv: Some(0),
            current_now_ua: Some(3_000_000),
            ..Default::default()
        };
        assert!(zero.negotiated_power_mw().is_none());

        let neg = TypeCPowerSupply {
            voltage_now_uv: Some(5_000_000),
            current_now_ua: Some(-1),
            ..Default::default()
        };
        assert!(neg.negotiated_power_mw().is_none());
    }

    #[test]
    fn current_roles_handle_brackets() {
        let p = TypeCPort {
            data_role: "host [device]".into(),
            power_role: "[source] sink".into(),
            ..Default::default()
        };
        assert_eq!(p.current_data_role(), "device");
        assert_eq!(p.current_power_role(), "source");
    }
}
