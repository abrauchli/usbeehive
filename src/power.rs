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
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

/// Manual impl so `parent_port_number` defaults to `-1` ("not linked") —
/// the derived `Default` would yield `0`, which is a valid port number and
/// spuriously pairs every unlinked PD node with Type-C port 0.
impl Default for PowerDeliveryPort {
    fn default() -> Self {
        PowerDeliveryPort {
            sysfs_path: PathBuf::default(),
            name: String::default(),
            parent_port_type: String::default(),
            parent_port_number: -1,
            source_capabilities: Vec::default(),
            sink_capabilities: Vec::default(),
            max_source_power_mw: 0,
            active_source_pdo_index: None,
            raw_attributes: BTreeMap::default(),
        }
    }
}

/// Voltage tolerance (mV) for matching a live measurement to an advertised
/// PDO. Real USB-PD rails drop a few hundred millivolts under load — a
/// 20V/5A fixed contract reads as anywhere from 19.5V to 20.0V depending
/// on cable resistance and current draw.
pub const PDO_MATCH_TOLERANCE_MV: u32 = 500;

impl PowerDeliveryPort {
    /// Mark the source PDO that matches `live_voltage_mv` as `is_active`.
    /// Returns the matched index in `source_capabilities`, or `None` when
    /// no PDO matches within [`PDO_MATCH_TOLERANCE_MV`].
    ///
    /// sysfs doesn't publish a per-PDO "active" flag — the kernel exposes
    /// every advertised PDO but not the contracted one. This helper bridges
    /// the gap by cross-referencing the live UCSI `voltage_now` reading
    /// against each PDO's voltage range.
    ///
    /// Match precedence:
    ///   1. Closest fixed-supply / battery PDO within tolerance.
    ///   2. PPS / Variable PDO whose `[voltage_mv, max_voltage_mv]` window
    ///      contains the live voltage.
    ///
    /// Re-runnable: any previously-active PDO is cleared before the new
    /// match is set.
    pub fn infer_active_source_pdo(&mut self, live_voltage_mv: u32) -> Option<usize> {
        if self.source_capabilities.is_empty() {
            return None;
        }

        // First pass: nearest fixed/battery PDO within tolerance.
        let fixed = self
            .source_capabilities
            .iter()
            .enumerate()
            .filter(|(_, p)| !matches!(p.r#type, PdoType::Pps | PdoType::VariableSupply))
            .filter(|(_, p)| p.voltage_mv.abs_diff(live_voltage_mv) <= PDO_MATCH_TOLERANCE_MV)
            .min_by_key(|(_, p)| p.voltage_mv.abs_diff(live_voltage_mv))
            .map(|(i, _)| i);

        let idx = fixed.or_else(|| {
            // Second pass: PPS / Variable whose [min, max] window contains
            // the live voltage (with tolerance on both ends).
            self.source_capabilities
                .iter()
                .enumerate()
                .find(|(_, p)| {
                    if !matches!(p.r#type, PdoType::Pps | PdoType::VariableSupply) {
                        return false;
                    }
                    let lo = p.voltage_mv.saturating_sub(PDO_MATCH_TOLERANCE_MV);
                    let hi = p
                        .max_voltage_mv
                        .max(p.voltage_mv)
                        .saturating_add(PDO_MATCH_TOLERANCE_MV);
                    live_voltage_mv >= lo && live_voltage_mv <= hi
                })
                .map(|(i, _)| i)
        })?;

        for p in self.source_capabilities.iter_mut() {
            p.is_active = false;
        }
        self.source_capabilities[idx].is_active = true;
        self.active_source_pdo_index = Some(self.source_capabilities[idx].index);
        Some(idx)
    }
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

    fn fixed(index: u32, voltage_mv: u32, current_ma: u32) -> PowerDataObject {
        PowerDataObject {
            r#type: PdoType::FixedSupply,
            voltage_mv,
            current_ma,
            power_mw: voltage_mv.saturating_mul(current_ma) / 1000,
            index,
            ..Default::default()
        }
    }

    #[test]
    fn infer_active_source_pdo_picks_exact_fixed() {
        let mut port = PowerDeliveryPort {
            source_capabilities: vec![
                fixed(1, 5_000, 3_000),
                fixed(2, 9_000, 3_000),
                fixed(3, 15_000, 3_000),
                fixed(4, 20_000, 5_000),
            ],
            ..Default::default()
        };
        // Live 20.0V → index 3.
        let i = port.infer_active_source_pdo(20_000).unwrap();
        assert_eq!(i, 3);
        assert!(port.source_capabilities[3].is_active);
        assert!(!port.source_capabilities[2].is_active);
        assert_eq!(port.active_source_pdo_index, Some(4));
    }

    #[test]
    fn infer_active_source_pdo_tolerates_voltage_drop() {
        let mut port = PowerDeliveryPort {
            source_capabilities: vec![fixed(1, 5_000, 3_000), fixed(2, 20_000, 5_000)],
            ..Default::default()
        };
        // 19.7V under load — should still match the 20V PDO.
        let i = port.infer_active_source_pdo(19_700).unwrap();
        assert_eq!(i, 1);
    }

    #[test]
    fn infer_active_source_pdo_matches_pps_range() {
        let pps = PowerDataObject {
            r#type: PdoType::Pps,
            voltage_mv: 3_300,
            max_voltage_mv: 21_000,
            current_ma: 5_000,
            power_mw: 100_000,
            index: 5,
            ..Default::default()
        };
        let mut port = PowerDeliveryPort {
            source_capabilities: vec![fixed(1, 5_000, 3_000), pps],
            ..Default::default()
        };
        // 12.0V inside the PPS [3.3, 21.0] window → PPS wins.
        let i = port.infer_active_source_pdo(12_000).unwrap();
        assert_eq!(i, 1);
        assert!(port.source_capabilities[1].is_active);
    }

    #[test]
    fn infer_active_source_pdo_prefers_fixed_over_pps_when_both_match() {
        // A 20V/5A live contract should match the fixed 20V PDO even if a
        // PPS window also covers it — fixed beats PPS for charging contracts.
        let pps = PowerDataObject {
            r#type: PdoType::Pps,
            voltage_mv: 3_300,
            max_voltage_mv: 21_000,
            index: 5,
            ..Default::default()
        };
        let mut port = PowerDeliveryPort {
            source_capabilities: vec![pps, fixed(2, 20_000, 5_000)],
            ..Default::default()
        };
        let i = port.infer_active_source_pdo(20_000).unwrap();
        assert_eq!(i, 1);
        assert!(!port.source_capabilities[0].is_active);
        assert!(port.source_capabilities[1].is_active);
    }

    #[test]
    fn infer_active_source_pdo_returns_none_when_nothing_matches() {
        let mut port = PowerDeliveryPort {
            source_capabilities: vec![fixed(1, 5_000, 3_000), fixed(2, 9_000, 3_000)],
            ..Default::default()
        };
        // 20V — no PDO advertises it.
        assert!(port.infer_active_source_pdo(20_000).is_none());
        assert!(port.source_capabilities.iter().all(|p| !p.is_active));
    }

    #[test]
    fn infer_active_source_pdo_clears_previous_active() {
        let mut port = PowerDeliveryPort {
            source_capabilities: vec![fixed(1, 5_000, 3_000), fixed(2, 20_000, 5_000)],
            ..Default::default()
        };
        port.source_capabilities[0].is_active = true; // stale
        let i = port.infer_active_source_pdo(20_000).unwrap();
        assert_eq!(i, 1);
        assert!(!port.source_capabilities[0].is_active);
        assert!(port.source_capabilities[1].is_active);
    }

    #[test]
    fn infer_active_source_pdo_handles_empty_list() {
        let mut port = PowerDeliveryPort::default();
        assert!(port.infer_active_source_pdo(20_000).is_none());
    }
}
