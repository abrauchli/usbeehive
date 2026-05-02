//! Charging-bottleneck diagnostic.
//!
//! Looks at a [`PowerDeliveryPort`] (charger advertisement + active contract)
//! plus the optional cable view, and produces a [`ChargingDiagnostic`] that
//! identifies the limiting factor.

use serde::Serialize;

use crate::cable::CableInfo;
use crate::power::PowerDeliveryPort;

/// Categorical reason a USB-C port may not be charging at full speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Bottleneck {
    /// No charger detected (or no source PDOs).
    NoCharger,
    /// Charger is offering less than expected.
    ChargerLimit,
    /// Cable e-marker advertises lower wattage than the charger can deliver.
    CableLimit,
    /// The downstream device is requesting less than the charger can deliver.
    DeviceLimit,
    /// Charging is at the charger's maximum.
    Fine,
}

/// Result of [`ChargingDiagnostic::evaluate`].
#[derive(Debug, Clone, Serialize)]
pub struct ChargingDiagnostic {
    /// Categorical bottleneck classification.
    pub bottleneck: Bottleneck,
    /// One-line headline (`"Cable is limiting charging speed"`).
    pub summary: String,
    /// Optional second-line detail.
    pub detail: String,
    /// `true` when the bottleneck is user-actionable (i.e. swap the cable).
    pub is_warning: bool,
}

impl ChargingDiagnostic {
    /// Compute a diagnostic for `pd_port`, optionally informed by `cable`.
    ///
    /// Returns `None` when no source capabilities are advertised — without
    /// that we have no charger reference to compare against.
    pub fn evaluate(
        pd_port: &PowerDeliveryPort,
        cable: Option<&CableInfo>,
    ) -> Option<ChargingDiagnostic> {
        if pd_port.source_capabilities.is_empty() {
            return None;
        }
        let charger_max_w = pd_port.max_source_power_mw / 1000;
        if charger_max_w == 0 {
            return None;
        }

        let active_w = pd_port
            .source_capabilities
            .iter()
            .find(|p| p.is_active)
            .map(|p| p.power_mw / 1000)
            .filter(|w| *w > 0)
            .unwrap_or(charger_max_w);

        let cable_max_w = cable.map(|c| c.max_watts).unwrap_or(0);

        if cable_max_w > 0 && cable_max_w < charger_max_w {
            Some(ChargingDiagnostic {
                bottleneck: Bottleneck::CableLimit,
                summary: "Cable is limiting charging speed".into(),
                detail: format!(
                    "Cable rated for {cable_max_w}W, but charger can deliver {charger_max_w}W"
                ),
                is_warning: true,
            })
        } else if active_w > 0 && (active_w as f64) < (charger_max_w as f64) * 0.8 {
            Some(ChargingDiagnostic {
                bottleneck: Bottleneck::DeviceLimit,
                summary: format!("Charging at {active_w}W"),
                detail: format!(
                    "Charging at {active_w}W (charger can do up to {charger_max_w}W)"
                ),
                is_warning: false,
            })
        } else {
            Some(ChargingDiagnostic {
                bottleneck: Bottleneck::Fine,
                summary: format!("Charging well at {active_w}W"),
                detail: String::new(),
                is_warning: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::{PdoType, PowerDataObject};

    fn pdo(power_mw: u32, active: bool) -> PowerDataObject {
        PowerDataObject {
            r#type: PdoType::FixedSupply,
            power_mw,
            is_active: active,
            ..Default::default()
        }
    }

    fn pd(caps: Vec<PowerDataObject>) -> PowerDeliveryPort {
        let max = caps.iter().map(|p| p.power_mw).max().unwrap_or(0);
        PowerDeliveryPort {
            source_capabilities: caps,
            max_source_power_mw: max,
            ..Default::default()
        }
    }

    #[test]
    fn no_source_caps_returns_none() {
        assert!(ChargingDiagnostic::evaluate(&PowerDeliveryPort::default(), None).is_none());
    }

    #[test]
    fn zero_charger_returns_none() {
        let port = pd(vec![pdo(0, false)]);
        assert!(ChargingDiagnostic::evaluate(&port, None).is_none());
    }

    #[test]
    fn cable_limit_flagged_with_warning() {
        let port = pd(vec![pdo(100_000, true)]);
        let cable = CableInfo {
            max_watts: 60,
            ..Default::default()
        };
        let d = ChargingDiagnostic::evaluate(&port, Some(&cable)).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::CableLimit);
        assert!(d.is_warning);
        assert!(d.detail.contains("60W"));
    }

    #[test]
    fn device_limit_when_active_below_80pct() {
        let port = pd(vec![pdo(100_000, false), pdo(15_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::DeviceLimit);
        assert!(!d.is_warning);
        assert!(d.summary.contains("15W"));
    }

    #[test]
    fn fine_when_active_meets_charger() {
        let port = pd(vec![pdo(60_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::Fine);
        assert!(d.summary.contains("60W"));
    }

    #[test]
    fn fine_when_no_active_pdo_uses_max() {
        let port = pd(vec![pdo(100_000, false)]);
        let cable = CableInfo {
            max_watts: 240,
            ..Default::default()
        };
        let d = ChargingDiagnostic::evaluate(&port, Some(&cable)).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::Fine);
        assert!(d.summary.contains("100W"));
    }
}
