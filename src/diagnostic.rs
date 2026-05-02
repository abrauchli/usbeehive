//! Charging-bottleneck diagnostic.

use serde::Serialize;

use crate::cable::CableInfo;
use crate::power::PowerDeliveryPort;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Bottleneck {
    NoCharger,
    ChargerLimit,
    CableLimit,
    DeviceLimit,
    Fine,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChargingDiagnostic {
    pub bottleneck: Bottleneck,
    pub summary: String,
    pub detail: String,
    pub is_warning: bool,
}

impl ChargingDiagnostic {
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
    fn cable_limit_flagged_with_warning() {
        let port = pd(vec![pdo(100_000, true)]); // 100W charger
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
        let port = pd(vec![pdo(100_000, false), pdo(15_000, true)]); // 15W active vs 100W max
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
}
