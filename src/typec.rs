//! USB Type-C port enumeration from `/sys/class/typec/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::sysfs;

const TYPEC_BASE: &str = "/sys/class/typec";

#[derive(Debug, Clone, Default, Serialize)]
pub struct TypeCIdentity {
    pub vendor_id: u16,
    pub product_id: u16,
    pub vdos: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TypeCPartner {
    pub r#type: String,
    pub identity: Option<TypeCIdentity>,
    pub raw_attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TypeCCable {
    pub r#type: String,
    pub plug_type: String,
    pub identity: Option<TypeCIdentity>,
    pub raw_attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TypeCPowerSupply {
    pub sysfs_path: PathBuf,
    pub name: String,
    pub online: bool,
    pub voltage_now_uv: Option<i64>,
    pub current_now_ua: Option<i64>,
    pub current_max_ua: Option<i64>,
    pub voltage_min_uv: Option<i64>,
    pub voltage_max_uv: Option<i64>,
    pub charge_type: String,
    pub usb_type: String,
    pub raw_attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TypeCPort {
    pub sysfs_path: PathBuf,
    pub port_name: String,
    pub port_number: i32,

    pub data_role: String,
    pub power_role: String,
    pub port_type: String,
    pub power_op_mode: String,
    pub orientation: String,
    pub pd_revision: String,
    pub usb_typec_rev: String,
    pub power_supply: Option<TypeCPowerSupply>,

    pub partner: Option<TypeCPartner>,
    pub cable: Option<TypeCCable>,

    pub raw_attributes: BTreeMap<String, String>,
}

impl TypeCPort {
    pub fn is_connected(&self) -> bool {
        self.partner.is_some() || self.cable.is_some()
    }

    /// `data_role` is reported as e.g. `"host [device]"`. Strip to the
    /// bracketed value, falling back to the raw string.
    pub fn current_data_role(&self) -> String {
        bracketed(&self.data_role)
    }

    pub fn current_power_role(&self) -> String {
        bracketed(&self.power_role)
    }

    pub fn enumerate() -> Vec<TypeCPort> {
        enumerate_in(Path::new(TYPEC_BASE))
    }
}

pub(crate) fn bracketed(s: &str) -> String {
    if let Some(start) = s.find('[') {
        if let Some(end) = s[start + 1..].find(']') {
            return s[start + 1..start + 1 + end].to_string();
        }
    }
    s.to_string()
}

fn read_identity(path: &Path) -> Option<TypeCIdentity> {
    let id_path = path.join("identity");
    if !sysfs::path_exists(&id_path) {
        return None;
    }
    let mut id = TypeCIdentity::default();
    if let Some(v) = sysfs::read_hex(id_path.join("id_header")) {
        id.vendor_id = (v & 0xFFFF) as u16;
    }
    if let Some(v) = sysfs::read_hex(id_path.join("product")) {
        id.product_id = (v & 0xFFFF) as u16;
    }
    let mut names: Vec<String> = std::fs::read_dir(&id_path)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for entry in names {
        let keep = entry.starts_with("vdo")
            || matches!(
                entry.as_str(),
                "id_header"
                    | "cert_stat"
                    | "product"
                    | "product_type_vdo1"
                    | "product_type_vdo2"
                    | "product_type_vdo3"
            );
        if !keep {
            continue;
        }
        if let Some(v) = sysfs::read_hex(id_path.join(&entry)) {
            id.vdos.push(v);
        }
    }
    if id.vendor_id == 0 && id.vdos.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn read_ucsi_power_supply(port_path: &Path, port_number: i32) -> Option<TypeCPowerSupply> {
    if port_number < 0 {
        return None;
    }
    let resolved = std::fs::canonicalize(port_path).ok()?;
    let s = resolved.to_string_lossy();
    let controller = ucsi_controller(&s)?;
    let psy_path =
        PathBuf::from(format!("/sys/class/power_supply/ucsi-source-psy-{controller}{}", port_number + 1));
    if !sysfs::path_exists(&psy_path) {
        return None;
    }
    let name = psy_path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(TypeCPowerSupply {
        sysfs_path: psy_path.clone(),
        name,
        online: sysfs::read_int(psy_path.join("online")).unwrap_or(0) != 0,
        voltage_now_uv: sysfs::read_int(psy_path.join("voltage_now")),
        current_now_ua: sysfs::read_int(psy_path.join("current_now")),
        current_max_ua: sysfs::read_int(psy_path.join("current_max")),
        voltage_min_uv: sysfs::read_int(psy_path.join("voltage_min")),
        voltage_max_uv: sysfs::read_int(psy_path.join("voltage_max")),
        charge_type: sysfs::read_attr(psy_path.join("charge_type")).unwrap_or_default(),
        usb_type: sysfs::read_attr(psy_path.join("usb_type")).unwrap_or_default(),
        raw_attributes: sysfs::read_all_attrs(&psy_path),
    })
}

/// Extract a UCSI controller id like `USBC000:00` from a canonical sysfs
/// path. Returns `None` when the path doesn't look like a UCSI device.
pub(crate) fn ucsi_controller(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let needle = b"USBC";
    let start = (0..bytes.len()).find(|&i| bytes[i..].starts_with(needle))?;
    let mut end = start + needle.len();
    let mut saw_colon = false;
    let mut hex_after_colon = 0usize;
    while end < bytes.len() {
        let c = bytes[end];
        if c == b':' && !saw_colon {
            saw_colon = true;
            end += 1;
            continue;
        }
        if c.is_ascii_hexdigit() {
            if saw_colon {
                hex_after_colon += 1;
            }
            end += 1;
        } else {
            break;
        }
    }
    if saw_colon && hex_after_colon > 0 {
        Some(path[start..end].to_string())
    } else {
        None
    }
}

fn from_sysfs(path: &Path, name: &str) -> Option<TypeCPort> {
    if !name.starts_with("port") {
        return None;
    }
    let port_number = name
        .trim_start_matches("port")
        .parse::<i32>()
        .ok()
        .filter(|_| name.bytes().skip(4).all(|b| b.is_ascii_digit()))
        .unwrap_or(-1);

    let mut port = TypeCPort {
        sysfs_path: path.to_path_buf(),
        port_name: name.to_string(),
        port_number,
        data_role: sysfs::read_attr(path.join("data_role")).unwrap_or_default(),
        power_role: sysfs::read_attr(path.join("power_role")).unwrap_or_default(),
        port_type: sysfs::read_attr(path.join("port_type")).unwrap_or_default(),
        power_op_mode: sysfs::read_attr(path.join("power_operation_mode")).unwrap_or_default(),
        orientation: sysfs::read_attr(path.join("orientation")).unwrap_or_default(),
        pd_revision: sysfs::read_attr(path.join("usb_power_delivery_revision")).unwrap_or_default(),
        usb_typec_rev: sysfs::read_attr(path.join("usb_typec_revision")).unwrap_or_default(),
        power_supply: read_ucsi_power_supply(path, port_number),
        partner: None,
        cable: None,
        raw_attributes: sysfs::read_all_attrs(path),
    };

    let partner_path = path.with_file_name(format!("{name}-partner"));
    if sysfs::path_exists(&partner_path) {
        port.partner = Some(TypeCPartner {
            r#type: sysfs::read_attr(partner_path.join("type")).unwrap_or_default(),
            identity: read_identity(&partner_path),
            raw_attributes: sysfs::read_all_attrs(&partner_path),
        });
    }
    let cable_path = path.with_file_name(format!("{name}-cable"));
    if sysfs::path_exists(&cable_path) {
        port.cable = Some(TypeCCable {
            r#type: sysfs::read_attr(cable_path.join("type")).unwrap_or_default(),
            plug_type: sysfs::read_attr(cable_path.join("plug_type")).unwrap_or_default(),
            identity: read_identity(&cable_path),
            raw_attributes: sysfs::read_all_attrs(&cable_path),
        });
    }

    Some(port)
}

fn enumerate_in(base: &Path) -> Vec<TypeCPort> {
    if !sysfs::path_exists(base) {
        return Vec::new();
    }
    let mut entries: Vec<(PathBuf, String)> = sysfs::subdirs(base)
        .into_iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            Some((p, name))
        })
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    entries
        .iter()
        .filter_map(|(p, n)| from_sysfs(p, n))
        .collect()
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
    fn ucsi_controller_extracts_id() {
        let p = "/sys/devices/platform/USBC000:00/typec/port0";
        assert_eq!(ucsi_controller(p).as_deref(), Some("USBC000:00"));
    }

    #[test]
    fn ucsi_controller_none_when_missing() {
        assert!(ucsi_controller("/sys/class/typec/port0").is_none());
        assert!(ucsi_controller("USBCnope").is_none());
    }

    #[test]
    fn is_connected_logic() {
        let mut p = TypeCPort::default();
        assert!(!p.is_connected());
        p.cable = Some(TypeCCable::default());
        assert!(p.is_connected());
    }
}
