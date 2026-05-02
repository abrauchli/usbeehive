//! USB Type-C port enumeration from `/sys/class/typec/`.

use std::path::{Path, PathBuf};

use whatcable_core::typec::{TypeCCable, TypeCIdentity, TypeCPartner, TypeCPort, TypeCPowerSupply};

use crate::sysfs::{self, Sysfs};

impl Sysfs {
    /// Walk this sysfs root's Type-C ports directory and return a snapshot.
    ///
    /// Many systems lack `/sys/class/typec` (it's published only when a
    /// supporting controller driver is loaded). In that case this returns
    /// an empty `Vec`.
    pub fn typec_ports(&self) -> Vec<TypeCPort> {
        let psy_root = self.power_supply_dir();
        enumerate_in(&self.typec_dir(), &psy_root)
    }
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
    // Push VDOs in USB-PD spec order — *not* alphabetical filename order,
    // which would put cert_stat ahead of id_header. Decoders rely on this
    // (CableInfo reads vdos[0] as the ID Header and vdos[3] as Cable VDO).
    const SPEC_ORDER: &[&str] = &[
        "id_header",
        "cert_stat",
        "product",
        "product_type_vdo1",
        "product_type_vdo2",
        "product_type_vdo3",
    ];
    for entry in SPEC_ORDER {
        if let Some(v) = sysfs::read_hex(id_path.join(entry)) {
            id.vdos.push(v);
        } else {
            // Preserve indexing for downstream consumers (vdos[3] must be
            // product_type_vdo1 for cables); pad with zero when a slot is
            // missing.
            id.vdos.push(0);
        }
    }
    while id.vdos.last() == Some(&0) {
        id.vdos.pop();
    }
    if id.vendor_id == 0 && id.vdos.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn read_ucsi_power_supply(
    port_path: &Path,
    port_number: i32,
    psy_root: &Path,
) -> Option<TypeCPowerSupply> {
    if port_number < 0 {
        return None;
    }
    let resolved = std::fs::canonicalize(port_path).ok()?;
    let s = resolved.to_string_lossy();
    let controller = ucsi_controller(&s)?;
    let psy_path = psy_root.join(format!(
        "ucsi-source-psy-{controller}{}",
        port_number + 1
    ));
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
pub fn ucsi_controller(path: &str) -> Option<String> {
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

fn from_sysfs(path: &Path, name: &str, psy_root: &Path) -> Option<TypeCPort> {
    if !name.starts_with("port") {
        return None;
    }
    let trail = name.trim_start_matches("port");
    // Reject sibling entries like `port0-partner` / `port0-cable` / `port0-plug0`
    // — only `port<digits>` should be treated as a port.
    if trail.is_empty() || !trail.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port_number = trail.parse::<i32>().unwrap_or(-1);

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
        power_supply: read_ucsi_power_supply(path, port_number, psy_root),
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

pub(crate) fn enumerate_in(base: &Path, psy_root: &Path) -> Vec<TypeCPort> {
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
        .filter_map(|(p, n)| from_sysfs(p, n, psy_root))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn missing_typec_dir_returns_empty() {
        let result = enumerate_in(Path::new("/no/such/whatcable/path"), Path::new("/no/such"));
        assert!(result.is_empty());
    }
}
