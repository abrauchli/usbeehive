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
        s if s.contains("pps") => PdoType::Pps,
        _ => PdoType::Unknown,
    }
}

fn parse_pdos(caps_path: &Path) -> Vec<PowerDataObject> {
    if !reader::path_exists(caps_path) {
        return Vec::new();
    }
    let mut entries = reader::subdirs(caps_path);
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let idx = name
                .rsplit_once(':')
                .map(|(_, t)| t)
                .unwrap_or(&name)
                .parse()
                .unwrap_or(0);
            let r#type = parse_pdo_type(&reader::read_attr(p.join("type")).unwrap_or_default());

            let voltage = reader::read_int(p.join("voltage")).map(|v| v as u32);
            let mut voltage_mv = voltage.unwrap_or(0);
            let max_voltage_mv = reader::read_int(p.join("maximum_voltage"))
                .map(|v| v as u32)
                .unwrap_or(0);
            // PPS / variable-supply PDOs don't publish a single `voltage`
            // file — they expose `minimum_voltage` + `maximum_voltage`. Use
            // the minimum as the canonical low end of the range.
            if voltage.is_none() {
                if let Some(v) = reader::read_int(p.join("minimum_voltage")) {
                    voltage_mv = v as u32;
                }
            }

            let current_ma = reader::read_int(p.join("maximum_current"))
                .or_else(|| reader::read_int(p.join("current")))
                .map(|v| v as u32)
                .unwrap_or(0);

            let power_mw = reader::read_int(p.join("maximum_power"))
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
    Some(PowerDeliveryPort {
        sysfs_path: path.to_path_buf(),
        name: name.to_string(),
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
        assert_eq!(parse_pdo_type("???"), PdoType::Unknown);
    }

    #[test]
    fn missing_pd_dir_returns_empty() {
        assert!(enumerate_in(Path::new("/no/such/usbeehive/path")).is_empty());
    }
}
