//! USB Power Delivery PDO data model.
//!
//! Each PDO advertised by a charger or sink corresponds to a numbered
//! sub-directory under `/sys/class/usb_power_delivery/<port>/source-capabilities/`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

/// Discriminator for [`PowerDataObject`] entries.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PdoType {
    /// Fixed-voltage source (the standard 5V / 9V / 15V / 20V profiles).
    FixedSupply,
    /// Battery-backed variable voltage.
    Battery,
    /// Variable-voltage supply with non-PPS regulation.
    VariableSupply,
    /// Programmable Power Supply (USB-PD 3.0 PPS / EPR APDO).
    Pps,
    /// Reserved or unrecognised.
    #[default]
    Unknown,
}

impl PdoType {
    /// Short human label (`"Fixed"` / `"Battery"` / `"Variable"` / `"PPS"`).
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

/// One PDO published by a USB-PD source or sink.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct PowerDataObject {
    /// Type discriminator parsed from the `type` sysfs file.
    pub r#type: PdoType,
    /// Voltage in **mV** (or minimum voltage for PPS / variable).
    pub voltage_mv: u32,
    /// Maximum voltage in **mV**, only meaningful for PPS / variable supplies.
    pub max_voltage_mv: u32,
    /// Maximum current in **mA**.
    pub current_ma: u32,
    /// Maximum power in **mW** (`voltage × current` if not provided).
    pub power_mw: u32,
    /// `true` if this PDO is the currently negotiated contract.
    pub is_active: bool,
    /// PDO index in the source/sink capabilities array.
    pub index: u32,
}

impl PowerDataObject {
    /// Voltage label, with a `"3.3-21.0V"`-style range for PPS.
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

    /// Two-decimal current label (e.g. `"3.00A"`).
    pub fn current_label(&self) -> String {
        format!("{:.2}A", self.current_ma as f64 / 1000.0)
    }

    /// Integer-watt power label (e.g. `"60W"`).
    pub fn power_label(&self) -> String {
        format!("{:.0}W", self.power_mw as f64 / 1000.0)
    }
}

/// All PDOs published by one USB Power Delivery port.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct PowerDeliveryPort {
    /// Absolute sysfs path of the port directory.
    pub sysfs_path: PathBuf,
    /// Sysfs name (e.g. `"pd0"`).
    pub name: String,
    /// Companion Type-C port type, when published as a `parent_port` attribute.
    pub parent_port_type: String,
    /// Companion Type-C port number, or `-1` if not linked.
    pub parent_port_number: i32,

    /// Source PDOs (what we can deliver as a source / what a charger advertises).
    pub source_capabilities: Vec<PowerDataObject>,
    /// Sink PDOs (what the partner is willing to accept).
    pub sink_capabilities: Vec<PowerDataObject>,
    /// Highest `power_mw` across `source_capabilities`.
    pub max_source_power_mw: u32,
    /// Index of the active source PDO, if any.
    pub active_source_pdo_index: Option<u32>,

    /// All regular files under the port directory.
    pub raw_attributes: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn current_and_power_labels() {
        let pdo = PowerDataObject {
            current_ma: 3000,
            power_mw: 60_000,
            ..Default::default()
        };
        assert_eq!(pdo.current_label(), "3.00A");
        assert_eq!(pdo.power_label(), "60W");
    }

    #[test]
    fn pdo_type_labels() {
        assert_eq!(PdoType::FixedSupply.label(), "Fixed");
        assert_eq!(PdoType::Pps.label(), "PPS");
        assert_eq!(PdoType::Unknown.label(), "Unknown");
    }
}
