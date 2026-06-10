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
    /// The negotiated contract is well below what the charger can deliver.
    DeviceLimit,
    /// The contract is healthy but the sink is requesting much less than
    /// it allows — typically a battery charge limit or thermal policy on
    /// the sink side, not a cable or charger problem.
    SinkLimit,
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
    /// Compute a diagnostic for `pd_port`, optionally informed by `cable`
    /// and the sink's requested operating power.
    ///
    /// `requested_mw` is the RDO operating power (`voltage_now ×
    /// current_now` off the UCSI power-supply) — the draw ceiling the sink
    /// asked for, **not** a measured flow. Pass `0` when unavailable.
    /// It distinguishes "the negotiation came out low" (cable/charger
    /// problem — what this app exists to catch) from "the contract is
    /// fine but the sink is asking for less" (benign sink policy, e.g. a
    /// battery charge limit).
    ///
    /// Returns `None` when no source capabilities are advertised — without
    /// that we have no charger reference to compare against.
    pub fn evaluate(
        pd_port: &PowerDeliveryPort,
        cable: Option<&CableInfo>,
        requested_mw: u32,
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
        let requested_w = requested_mw / 1000;

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
                summary: format!("Contract limited to {active_w}W"),
                detail: format!(
                    "Negotiated {active_w}W of the {charger_max_w}W the charger offers"
                ),
                is_warning: false,
            })
        } else if requested_w > 0 && (requested_w as f64) < (active_w as f64) * 0.8 {
            // Contract is healthy — the sink itself is asking for less.
            Some(ChargingDiagnostic {
                bottleneck: Bottleneck::SinkLimit,
                summary: format!("Device is limiting its draw to {requested_w}W"),
                detail: format!(
                    "Contract allows {active_w}W; the device requests up to {requested_w}W — \
                     often a battery charge limit or thermal policy"
                ),
                is_warning: false,
            })
        } else {
            Some(ChargingDiagnostic {
                bottleneck: Bottleneck::Fine,
                summary: format!("Charging at up to {active_w}W"),
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
        assert!(ChargingDiagnostic::evaluate(&PowerDeliveryPort::default(), None, 0).is_none());
    }

    #[test]
    fn zero_charger_returns_none() {
        let port = pd(vec![pdo(0, false)]);
        assert!(ChargingDiagnostic::evaluate(&port, None, 0).is_none());
    }

    #[test]
    fn cable_limit_flagged_with_warning() {
        let port = pd(vec![pdo(100_000, true)]);
        let cable = CableInfo {
            max_watts: 60,
            ..Default::default()
        };
        let d = ChargingDiagnostic::evaluate(&port, Some(&cable), 0).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::CableLimit);
        assert!(d.is_warning);
        assert!(d.detail.contains("60W"));
    }

    #[test]
    fn device_limit_when_active_below_80pct() {
        let port = pd(vec![pdo(100_000, false), pdo(15_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None, 0).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::DeviceLimit);
        assert!(!d.is_warning);
        assert!(d.summary.contains("15W"));
    }

    #[test]
    fn fine_when_active_meets_charger() {
        let port = pd(vec![pdo(60_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None, 0).unwrap();
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
        let d = ChargingDiagnostic::evaluate(&port, Some(&cable), 0).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::Fine);
        assert!(d.summary.contains("100W"));
    }

    #[test]
    fn sink_limit_when_request_far_below_healthy_contract() {
        // 65W charger, full 65W contract negotiated, but the sink only
        // requests 15W in its RDO (e.g. a laptop with an 80% battery
        // charge limit). Benign — must not raise is_warning.
        let port = pd(vec![pdo(65_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None, 15_000).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::SinkLimit);
        assert!(!d.is_warning);
        assert!(d.summary.contains("15W"));
        assert!(d.detail.contains("65W"));
    }

    #[test]
    fn sink_limit_not_raised_when_request_near_contract() {
        // Requesting 60W of a 65W contract (> 80%) is Fine, not SinkLimit.
        let port = pd(vec![pdo(65_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None, 60_000).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::Fine);
    }

    #[test]
    fn cable_limit_wins_over_sink_limit() {
        // A bad cable is the actionable fact even when the sink also
        // happens to be requesting little.
        let port = pd(vec![pdo(100_000, true)]);
        let cable = CableInfo {
            max_watts: 60,
            ..Default::default()
        };
        let d = ChargingDiagnostic::evaluate(&port, Some(&cable), 10_000).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::CableLimit);
        assert!(d.is_warning);
    }

    #[test]
    fn device_limit_wins_over_sink_limit() {
        // Contract itself is low (15W of 100W) — that's the dominant
        // story even if the request is lower still.
        let port = pd(vec![pdo(100_000, false), pdo(15_000, true)]);
        let d = ChargingDiagnostic::evaluate(&port, None, 5_000).unwrap();
        assert_eq!(d.bottleneck, Bottleneck::DeviceLimit);
    }
}
