//! USB Power Delivery PDO enumeration from `/sys/class/usb_power_delivery/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::sysfs;

const PD_BASE: &str = "/sys/class/usb_power_delivery";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PdoType {
    FixedSupply,
    Battery,
    VariableSupply,
    Pps,
    Unknown,
}

impl PdoType {
    pub fn label(self) -> &'static str {
        match self {
            PdoType::FixedSupply => "Fixed",
            PdoType::Battery => "Battery",
            PdoType::VariableSupply => "Variable",
            PdoType::Pps => "PPS",
            PdoType::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PowerDataObject {
    pub r#type: PdoType,
    pub voltage_mv: u32,
    /// Set for PPS / variable supplies, otherwise zero.
    pub max_voltage_mv: u32,
    pub current_ma: u32,
    pub power_mw: u32,
    pub is_active: bool,
    pub index: u32,
}

impl Default for PdoType {
    fn default() -> Self {
        PdoType::Unknown
    }
}

impl PowerDataObject {
    pub fn voltage_label(&self) -> String {
        if matches!(self.r#type, PdoType::Pps) && self.max_voltage_mv > 0 {
            format!(
                "{:.1}-{:.1}V",
                self.voltage_mv as f64 / 1000.0,
                self.max_voltage_mv as f64 / 1000.0
            )
        } else {
            format!("{:.1}V", self.voltage_mv as f64 / 1000.0)
        }
    }

    pub fn current_label(&self) -> String {
        format!("{:.2}A", self.current_ma as f64 / 1000.0)
    }

    pub fn power_label(&self) -> String {
        format!("{:.0}W", self.power_mw as f64 / 1000.0)
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PowerDeliveryPort {
    pub sysfs_path: PathBuf,
    pub name: String,
    pub parent_port_type: String,
    pub parent_port_number: i32,

    pub source_capabilities: Vec<PowerDataObject>,
    pub sink_capabilities: Vec<PowerDataObject>,
    pub max_source_power_mw: u32,
    pub active_source_pdo_index: Option<u32>,

    pub raw_attributes: BTreeMap<String, String>,
}

impl PowerDeliveryPort {
    pub fn enumerate() -> Vec<PowerDeliveryPort> {
        enumerate_in(Path::new(PD_BASE))
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
    if !sysfs::path_exists(caps_path) {
        return Vec::new();
    }
    let mut entries = sysfs::subdirs(caps_path);
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let idx = name
                .rsplit_once(':')
                .map(|(_, t)| t)
                .unwrap_or(&name)
                .parse()
                .unwrap_or(0);
            let r#type = parse_pdo_type(&sysfs::read_attr(p.join("type")).unwrap_or_default());

            let voltage = sysfs::read_int(p.join("voltage")).map(|v| v as u32);
            let mut voltage_mv = voltage.unwrap_or(0);
            let max_voltage_mv = sysfs::read_int(p.join("maximum_voltage"))
                .map(|v| v as u32)
                .unwrap_or(0);
            if max_voltage_mv == 0 && voltage.is_none() {
                if let Some(v) = sysfs::read_int(p.join("minimum_voltage")) {
                    voltage_mv = v as u32;
                }
            }

            let current_ma = sysfs::read_int(p.join("maximum_current"))
                .or_else(|| sysfs::read_int(p.join("current")))
                .map(|v| v as u32)
                .unwrap_or(0);

            let power_mw = sysfs::read_int(p.join("maximum_power"))
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
        raw_attributes: sysfs::read_all_attrs(path),
        ..Default::default()
    })
}

fn enumerate_in(base: &Path) -> Vec<PowerDeliveryPort> {
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
    fn pdo_type_parses_known_strings() {
        assert_eq!(parse_pdo_type("fixed_supply"), PdoType::FixedSupply);
        assert_eq!(parse_pdo_type("battery"), PdoType::Battery);
        assert_eq!(parse_pdo_type("variable_supply"), PdoType::VariableSupply);
        assert_eq!(parse_pdo_type("programmable_supply (pps)"), PdoType::Pps);
        assert_eq!(parse_pdo_type("???"), PdoType::Unknown);
    }

    #[test]
    fn voltage_label_pps_range() {
        let pdo = PowerDataObject {
            r#type: PdoType::Pps,
            voltage_mv: 3300,
            max_voltage_mv: 21000,
            ..Default::default()
        };
        assert_eq!(pdo.voltage_label(), "3.3-21.0V");
    }

    #[test]
    fn voltage_label_fixed() {
        let pdo = PowerDataObject {
            r#type: PdoType::FixedSupply,
            voltage_mv: 9000,
            ..Default::default()
        };
        assert_eq!(pdo.voltage_label(), "9.0V");
        assert_eq!(
            PowerDataObject {
                current_ma: 3000,
                ..Default::default()
            }
            .current_label(),
            "3.00A"
        );
        assert_eq!(
            PowerDataObject {
                power_mw: 60000,
                ..Default::default()
            }
            .power_label(),
            "60W"
        );
    }
}
