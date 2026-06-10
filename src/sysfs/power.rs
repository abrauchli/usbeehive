//! USB Power Delivery PDO enumeration from `/sys/class/usb_power_delivery/`.

use std::path::{Path, PathBuf};

use crate::power::{PdoType, PowerDataObject, PowerDeliveryPort};

use super::reader::{self, Sysfs};

impl Sysfs {
    /// Walk this sysfs root's PD ports directory and return a snapshot.
    ///
    /// As with Type-C, this directory is published only when a controller
    /// driver loads it; missing directory yields an empty `Vec`.
    pub fn pd_ports(&self) -> Vec<PowerDeliveryPort> {
        enumerate_in(&self.pd_dir())
    }
}

fn parse_pdo_type(raw: &str) -> PdoType {
    match raw {
        "fixed_supply" => PdoType::FixedSupply,
        "battery" => PdoType::Battery,
        "variable_supply" => PdoType::VariableSupply,
        // The kernel's directory-name form is `programmable_supply`; older
        // descriptions spell out `... (pps)`.
        s if s.contains("pps") || s.contains("programmable") => PdoType::Pps,
        _ => PdoType::Unknown,
    }
}

/// Parse the leading base-10 digits of a PDO attribute. The kernel's typec
/// pd class formats these values with unit suffixes — `5000mV` / `3000mA`
/// / `45000mW` (see `drivers/usb/typec/pd.c`) — unlike most integer sysfs
/// attributes. Bare integers (the fixture convention) parse too.
fn read_pdo_int(path: PathBuf) -> Option<i64> {
    let s = reader::read_attr(path)?;
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s[..end].parse().ok()
}

fn parse_pdos(caps_path: &Path) -> Vec<PowerDataObject> {
    if !reader::path_exists(caps_path) {
        return Vec::new();
    }
    let mut entries = reader::subdirs(caps_path);
    // Keep only PDO entries (named `<index>:<type>`) — the kernel also
    // places a runtime-PM `power` directory inside `*-capabilities/`,
    // which must not become a junk all-zero PDO.
    entries.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(|c: char| c.is_ascii_digit()))
    });
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let (idx_part, type_part) = name.split_once(':').unwrap_or((name.as_str(), ""));
            let idx = idx_part.parse().unwrap_or(0);
            // Real kernels publish no `type` attribute — the PDO type is
            // encoded in the directory name (`5:programmable_supply`). The
            // explicit attribute (fixture convention) wins when present.
            let r#type = reader::read_attr(p.join("type"))
                .map(|s| parse_pdo_type(&s))
                .unwrap_or_else(|| parse_pdo_type(type_part));

            let voltage = read_pdo_int(p.join("voltage")).map(|v| v as u32);
            let mut voltage_mv = voltage.unwrap_or(0);
            let max_voltage_mv = read_pdo_int(p.join("maximum_voltage"))
                .map(|v| v as u32)
                .unwrap_or(0);
            // PPS / variable-supply PDOs don't publish a single `voltage`
            // file — they expose `minimum_voltage` + `maximum_voltage`. Use
            // the minimum as the canonical low end of the range.
            if voltage.is_none() {
                if let Some(v) = read_pdo_int(p.join("minimum_voltage")) {
                    voltage_mv = v as u32;
                }
            }

            let current_ma = read_pdo_int(p.join("maximum_current"))
                .or_else(|| read_pdo_int(p.join("current")))
                .map(|v| v as u32)
                .unwrap_or(0);

            let power_mw = read_pdo_int(p.join("maximum_power"))
                .map(|v| v as u32)
                .unwrap_or_else(|| {
                    if voltage_mv > 0 && current_ma > 0 {
                        ((voltage_mv as u64 * current_ma as u64) / 1000) as u32
                    } else {
                        0
                    }
                });

            PowerDataObject {
                r#type,
                voltage_mv,
                max_voltage_mv,
                current_ma,
                power_mw,
                is_active: false,
                index: idx,
            }
        })
        .collect()
}

fn from_sysfs(path: &Path, name: &str) -> Option<PowerDeliveryPort> {
    let source_capabilities = parse_pdos(&path.join("source-capabilities"));
    let sink_capabilities = parse_pdos(&path.join("sink-capabilities"));
    if source_capabilities.is_empty() && sink_capabilities.is_empty() {
        return None;
    }
    let max_source_power_mw = source_capabilities
        .iter()
        .map(|p| p.power_mw)
        .max()
        .unwrap_or(0);
    // `parent_port_number` is the fixture convention for port linkage —
    // real kernels publish no `parent_port*` attribute on `pdN` nodes
    // (linkage flows through the partner's `usb_power_delivery` symlink
    // instead). Default to -1 so an unlinked node never matches a port.
    let parent_port_number = reader::read_int(path.join("parent_port_number"))
        .map(|v| v as i32)
        .unwrap_or(-1);
    Some(PowerDeliveryPort {
        sysfs_path: path.to_path_buf(),
        name: name.to_string(),
        parent_port_number,
        source_capabilities,
        sink_capabilities,
        max_source_power_mw,
        raw_attributes: reader::read_all_attrs(path),
        ..Default::default()
    })
}

pub(crate) fn enumerate_in(base: &Path) -> Vec<PowerDeliveryPort> {
    if !reader::path_exists(base) {
        return Vec::new();
    }
    let mut entries: Vec<(PathBuf, String)> = reader::subdirs(base)
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
    fn pdo_type_parses_known_strings() {
        assert_eq!(parse_pdo_type("fixed_supply"), PdoType::FixedSupply);
        assert_eq!(parse_pdo_type("battery"), PdoType::Battery);
        assert_eq!(parse_pdo_type("variable_supply"), PdoType::VariableSupply);
        assert_eq!(parse_pdo_type("programmable_supply (pps)"), PdoType::Pps);
        assert_eq!(parse_pdo_type("programmable_supply"), PdoType::Pps);
        assert_eq!(parse_pdo_type("???"), PdoType::Unknown);
    }

    #[test]
    fn missing_pd_dir_returns_empty() {
        assert!(enumerate_in(Path::new("/no/such/usbeehive/path")).is_empty());
    }

    #[test]
    fn parse_pdos_tolerates_unit_suffixes_and_skips_power_dir() {
        // Real kernels format PDO values with unit suffixes (`5000mV`,
        // `3000mA`) and place a runtime-PM `power` directory inside the
        // capabilities directory; neither must derail parsing.
        let dir = std::env::temp_dir().join(format!("usbeehive-pdo-suffix-{}", std::process::id()));
        let caps = dir.join("source-capabilities");
        let pdo = caps.join("1:fixed_supply");
        std::fs::create_dir_all(&pdo).unwrap();
        std::fs::write(pdo.join("type"), "fixed_supply\n").unwrap();
        std::fs::write(pdo.join("voltage"), "5000mV\n").unwrap();
        std::fs::write(pdo.join("maximum_current"), "3000mA\n").unwrap();
        std::fs::create_dir_all(caps.join("power")).unwrap();

        // PPS entry the way real kernels publish it: no `type` attribute
        // (the type lives in the directory name), range voltages only.
        let pps = caps.join("5:programmable_supply");
        std::fs::create_dir_all(&pps).unwrap();
        std::fs::write(pps.join("minimum_voltage"), "5000mV\n").unwrap();
        std::fs::write(pps.join("maximum_voltage"), "21000mV\n").unwrap();
        std::fs::write(pps.join("maximum_current"), "3000mA\n").unwrap();

        let pdos = parse_pdos(&caps);
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(pdos.len(), 2, "the `power` dir must not become a PDO");
        assert_eq!(pdos[0].r#type, PdoType::FixedSupply);
        assert_eq!(pdos[0].index, 1);
        assert_eq!(pdos[0].voltage_mv, 5_000);
        assert_eq!(pdos[0].current_ma, 3_000);
        assert_eq!(pdos[0].power_mw, 15_000);
        assert_eq!(pdos[1].r#type, PdoType::Pps);
        assert_eq!(pdos[1].index, 5);
        assert_eq!(pdos[1].voltage_mv, 5_000);
        assert_eq!(pdos[1].max_voltage_mv, 21_000);
    }
}
