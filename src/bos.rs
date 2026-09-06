//! Binary Object Store (BOS) descriptor decoding.
//!
//! `/sys/bus/usb/devices/<dev>/bos_descriptors` is a raw binary blob holding
//! the device's Binary Object Store — the set of USB 3.x *Device Capability*
//! descriptors the device published at enumeration. Where `speed`, `rx_lanes`
//! and `tx_lanes` report what the link **negotiated**, the BOS reports what
//! the device is **capable of**, independent of the port it happens to be
//! plugged into.
//!
//! Everything here is pure: [`parse`] takes `&[u8]` and does no IO, so the
//! module is compiled in every feature configuration (alongside [`crate::pd`]
//! and [`crate::usb`]).
//!
//! # Absence is normal
//!
//! USB 2.0-only devices publish no BOS at all and the sysfs file simply does
//! not exist. Callers get `None` — never an error.
//!
//! # Robustness
//!
//! Blobs come from untrusted hardware. [`parse`] never panics and never
//! loops: a descriptor claiming `bLength < 3` (including the pathological
//! `0`) terminates the walk, as does any descriptor that would run past the
//! end of the blob. Whatever was decoded before the fault is returned with
//! [`BosDescriptors::truncated`] set.
//!
//! ```
//! use usbeehive::bos;
//!
//! // Real blob from a "USB 10/100/1000 LAN" adapter.
//! let blob = [
//!     0x05, 0x0f, 0x16, 0x00, 0x02, // BOS header: 2 capabilities
//!     0x07, 0x10, 0x02, 0x06, 0x00, 0x00, 0x00, // USB 2.0 Extension
//!     0x0a, 0x10, 0x03, 0x02, 0x0e, 0x00, 0x02, 0x0a, 0xff, 0x07, // SuperSpeed
//! ];
//! let bos = bos::parse(&blob).unwrap();
//! assert_eq!(bos.capabilities.len(), 2);
//! // Advertises SuperSpeed …
//! assert_eq!(bos.max_capable_mbps(), 5_000);
//! // … but declares full functionality already at High Speed.
//! assert_eq!(bos.functional_floor_mbps(), Some(480));
//! ```

use serde::Serialize;

/// `bDescriptorType` of the BOS root descriptor.
const DT_BOS: u8 = 0x0F;
/// `bDescriptorType` of every Device Capability descriptor inside a BOS.
const DT_DEVICE_CAPABILITY: u8 = 0x10;

/// Length of the BOS root descriptor itself.
const BOS_HEADER_LEN: usize = 5;

/// Hard ceiling on the number of capability descriptors decoded from one
/// blob. A well-formed BOS holds a handful; the cap is belt-and-braces
/// against a hostile blob, on top of the strictly-increasing offset walk.
const MAX_CAPABILITIES: usize = 64;

/// `bDevCapabilityType` values decoded into typed variants.
const CAP_USB_2_0_EXTENSION: u8 = 0x02;
const CAP_SUPERSPEED_USB: u8 = 0x03;
const CAP_CONTAINER_ID: u8 = 0x04;
const CAP_SUPERSPEED_PLUS: u8 = 0x0A;

/// Speed the device advertises via `wSpeedsSupported` bit 0 (Low Speed,
/// nominally 1.5 Mbps). Reported as `2` so it lands in the crate's
/// [`crate::usb::LinkSpeed::Low`] bucket, which is keyed on `>= 2` because
/// sysfs writes the un-parseable string `"1.5"` for this tier.
const SPEED_LOW_MBPS: u32 = 2;
const SPEED_FULL_MBPS: u32 = 12;
const SPEED_HIGH_MBPS: u32 = 480;
const SPEED_GEN1_MBPS: u32 = 5_000;

/// Decoded USB 2.0 Extension capability (`bDevCapabilityType == 0x02`).
///
/// Describes Link Power Management support. Carries no data-rate
/// information; decoded because it is the most common capability and
/// swallowing it as `Other` would lose the LPM/BESL detail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Usb2Extension {
    /// Raw `bmAttributes` (little-endian u32) as published.
    pub bm_attributes: u32,
    /// Bit 1 — Link Power Management supported.
    pub lpm: bool,
    /// Bit 2 — BESL and Alternate HIRD definitions supported.
    pub besl: bool,
    /// Bit 3 — the `baseline_besl` field carries a valid value.
    pub baseline_besl_valid: bool,
    /// Bit 4 — the `deep_besl` field carries a valid value.
    pub deep_besl_valid: bool,
    /// Bits 11:8 — recommended Baseline BESL value.
    pub baseline_besl: u8,
    /// Bits 15:12 — recommended Deep BESL value.
    pub deep_besl: u8,
}

impl Usb2Extension {
    fn from_bits(bm_attributes: u32) -> Self {
        Usb2Extension {
            bm_attributes,
            lpm: bm_attributes & (1 << 1) != 0,
            besl: bm_attributes & (1 << 2) != 0,
            baseline_besl_valid: bm_attributes & (1 << 3) != 0,
            deep_besl_valid: bm_attributes & (1 << 4) != 0,
            baseline_besl: ((bm_attributes >> 8) & 0xF) as u8,
            deep_besl: ((bm_attributes >> 12) & 0xF) as u8,
        }
    }
}

/// Decoded SuperSpeed USB Device Capability (`bDevCapabilityType == 0x03`).
///
/// This is the descriptor that answers "what speeds can this device do at
/// all", via the `wSpeedsSupported` bitmap, *and* "what is the slowest speed
/// at which the vendor considers the device fully functional", via
/// `bFunctionalitySupport`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SuperSpeedCapability {
    /// `bmAttributes` bit 1 — Latency Tolerance Messaging capable.
    pub ltm_capable: bool,
    /// Raw `wSpeedsSupported` bitmap. Bit 0 = low, 1 = full, 2 = high,
    /// 3 = SuperSpeed Gen 1 x1.
    pub speeds_supported: u16,
    /// `bFunctionalitySupport` — the **bit position** in `speeds_supported`
    /// of the lowest speed at which all device functionality is available.
    pub functionality_support: u8,
    /// `bU1DevExitLat`, in microseconds.
    pub u1_dev_exit_lat_us: u8,
    /// `wU2DevExitLat`, in microseconds.
    pub u2_dev_exit_lat_us: u16,
}

impl SuperSpeedCapability {
    /// Bit 0 — Low Speed supported.
    pub fn supports_low(&self) -> bool {
        self.speeds_supported & (1 << 0) != 0
    }
    /// Bit 1 — Full Speed supported.
    pub fn supports_full(&self) -> bool {
        self.speeds_supported & (1 << 1) != 0
    }
    /// Bit 2 — High Speed supported.
    pub fn supports_high(&self) -> bool {
        self.speeds_supported & (1 << 2) != 0
    }
    /// Bit 3 — SuperSpeed (Gen 1 x1, 5 Gbps) supported.
    pub fn supports_gen1(&self) -> bool {
        self.speeds_supported & (1 << 3) != 0
    }

    /// Highest speed in `wSpeedsSupported`, in Mbps. Zero when the bitmap is
    /// empty or holds only bits this decoder does not recognise.
    pub fn max_speed_mbps(&self) -> u32 {
        if self.supports_gen1() {
            SPEED_GEN1_MBPS
        } else if self.supports_high() {
            SPEED_HIGH_MBPS
        } else if self.supports_full() {
            SPEED_FULL_MBPS
        } else if self.supports_low() {
            SPEED_LOW_MBPS
        } else {
            0
        }
    }

    /// The vendor's declared floor for *full* functionality, in Mbps.
    ///
    /// `bFunctionalitySupport` is a bit index into `wSpeedsSupported`.
    /// Returns `None` when the index does not name a speed this decoder
    /// knows, or when the corresponding `wSpeedsSupported` bit is not
    /// actually set (a self-contradictory descriptor we refuse to trust).
    pub fn functional_floor_mbps(&self) -> Option<u32> {
        let mbps = match self.functionality_support {
            0 => SPEED_LOW_MBPS,
            1 => SPEED_FULL_MBPS,
            2 => SPEED_HIGH_MBPS,
            3 => SPEED_GEN1_MBPS,
            _ => return None,
        };
        if self.speeds_supported & (1 << self.functionality_support) == 0 {
            return None;
        }
        Some(mbps)
    }
}

/// `LSE` — the exponent applied to a sublink's Lane Speed Mantissa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LaneSpeedExponent {
    /// Mantissa is in bits per second.
    Bps,
    /// Mantissa is in kilobits per second.
    Kbps,
    /// Mantissa is in megabits per second.
    Mbps,
    /// Mantissa is in gigabits per second.
    Gbps,
}

impl LaneSpeedExponent {
    fn from_bits(bits: u8) -> Self {
        match bits & 0x3 {
            0 => LaneSpeedExponent::Bps,
            1 => LaneSpeedExponent::Kbps,
            2 => LaneSpeedExponent::Mbps,
            _ => LaneSpeedExponent::Gbps,
        }
    }
}

/// `ST` — whether a sublink speed attribute describes the receive or the
/// transmit direction, and whether the link is symmetric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SublinkType {
    /// Symmetric link, receive direction.
    SymmetricRx,
    /// Symmetric link, transmit direction.
    SymmetricTx,
    /// Asymmetric link, receive direction.
    AsymmetricRx,
    /// Asymmetric link, transmit direction.
    AsymmetricTx,
}

impl SublinkType {
    fn from_bits(bits: u8) -> Self {
        // bit 0 of the field: 0 = symmetric, 1 = asymmetric.
        // bit 1 of the field: 0 = receive,   1 = transmit.
        match bits & 0x3 {
            0b00 => SublinkType::SymmetricRx,
            0b01 => SublinkType::AsymmetricRx,
            0b10 => SublinkType::SymmetricTx,
            _ => SublinkType::AsymmetricTx,
        }
    }

    /// `true` for the two transmit variants.
    pub fn is_tx(self) -> bool {
        matches!(self, SublinkType::SymmetricTx | SublinkType::AsymmetricTx)
    }

    /// `true` for the two receive variants.
    pub fn is_rx(self) -> bool {
        !self.is_tx()
    }
}

/// `LP` — the link protocol a sublink speed attribute applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LinkProtocol {
    /// SuperSpeed (Gen 1 signalling).
    SuperSpeed,
    /// SuperSpeedPlus (Gen 2 and later signalling).
    SuperSpeedPlus,
    /// Reserved encoding this decoder does not recognise.
    Reserved(u8),
}

impl LinkProtocol {
    fn from_bits(bits: u8) -> Self {
        match bits & 0x3 {
            0 => LinkProtocol::SuperSpeed,
            1 => LinkProtocol::SuperSpeedPlus,
            other => LinkProtocol::Reserved(other),
        }
    }
}

/// One 32-bit entry of the SuperSpeedPlus `bmSublinkSpeedAttr[]` array.
///
/// This array is the only honest way to tell Gen 1x2 (two 5 Gbps lanes)
/// from Gen 2x1 (one 10 Gbps lane): both aggregate to 10 Gbps and both are
/// reported as `speed == 10000` by sysfs, but their per-lane mantissa and
/// lane counts differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SublinkSpeedAttribute {
    /// Bits 3:0 — Sublink Speed Attribute ID, referenced by
    /// [`SuperSpeedPlusCapability::min_functional_ssid`] and by the pairing
    /// of Rx/Tx entries.
    pub ssid: u8,
    /// Bits 5:4 — Lane Speed Exponent.
    pub exponent: LaneSpeedExponent,
    /// Bits 7:6 — Sublink Type (direction + symmetry).
    pub sublink_type: SublinkType,
    /// Bits 15:14 — Link Protocol.
    pub protocol: LinkProtocol,
    /// Bits 31:16 — Lane Speed Mantissa.
    pub mantissa: u16,
}

impl SublinkSpeedAttribute {
    fn from_bits(raw: u32) -> Self {
        SublinkSpeedAttribute {
            ssid: (raw & 0xF) as u8,
            exponent: LaneSpeedExponent::from_bits(((raw >> 4) & 0x3) as u8),
            sublink_type: SublinkType::from_bits(((raw >> 6) & 0x3) as u8),
            protocol: LinkProtocol::from_bits(((raw >> 14) & 0x3) as u8),
            mantissa: ((raw >> 16) & 0xFFFF) as u16,
        }
    }

    /// Speed of a **single lane** of this sublink, in Mbps. Saturates rather
    /// than overflowing on an absurd mantissa.
    pub fn lane_speed_mbps(&self) -> u32 {
        let m = self.mantissa as u64;
        let mbps = match self.exponent {
            LaneSpeedExponent::Bps => m / 1_000_000,
            LaneSpeedExponent::Kbps => m / 1_000,
            LaneSpeedExponent::Mbps => m,
            LaneSpeedExponent::Gbps => m.saturating_mul(1_000),
        };
        mbps.min(u32::MAX as u64) as u32
    }
}

/// Decoded SuperSpeedPlus USB Device Capability
/// (`bDevCapabilityType == 0x0A`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SuperSpeedPlusCapability {
    /// `bmAttributes` bits 4:0 (`SSAC`) **+ 1** — the number of entries the
    /// descriptor claims for `sublink_attrs`.
    pub sublink_speed_attr_count: u8,
    /// `bmAttributes` bits 8:5 (`SSIC`) **+ 1** — the number of distinct
    /// Sublink Speed IDs used.
    pub sublink_speed_id_count: u8,
    /// `wFunctionalitySupport` bits 3:0 — the `ssid` of the lowest sublink
    /// speed needed for full device functionality.
    pub min_functional_ssid: u8,
    /// `wFunctionalitySupport` bits 11:8 — minimum receive lane count.
    pub min_rx_lane_count: u8,
    /// `wFunctionalitySupport` bits 15:12 — minimum transmit lane count.
    pub min_tx_lane_count: u8,
    /// Decoded `bmSublinkSpeedAttr[]`. May hold fewer entries than
    /// `sublink_speed_attr_count` claims when the blob was truncated.
    pub sublink_attrs: Vec<SublinkSpeedAttribute>,
}

impl SuperSpeedPlusCapability {
    /// Fastest single-lane speed advertised, in Mbps (both directions
    /// considered). Zero when the attribute array is empty.
    pub fn max_lane_speed_mbps(&self) -> u32 {
        self.sublink_attrs
            .iter()
            .map(|a| a.lane_speed_mbps())
            .max()
            .unwrap_or(0)
    }

    /// Lane count to pair with [`Self::max_lane_speed_mbps`] — the smaller of
    /// the declared minimum Rx and Tx lane counts, floored at 1 so a
    /// descriptor that leaves the field zero still yields a usable figure.
    pub fn effective_lane_count(&self) -> u32 {
        let rx = self.min_rx_lane_count.max(1) as u32;
        let tx = self.min_tx_lane_count.max(1) as u32;
        rx.min(tx)
    }

    /// Aggregate capability across all lanes, in Mbps.
    pub fn max_capable_mbps(&self) -> u32 {
        self.max_lane_speed_mbps()
            .saturating_mul(self.effective_lane_count())
    }

    /// USB-IF generation label — `"Gen 1x1"`, `"Gen 1x2"`, `"Gen 2x1"`,
    /// `"Gen 2x2"`, `"Gen 3x1"`, `"Gen 3x2"` — or `None` when the per-lane
    /// speed is not one this decoder can name.
    ///
    /// This is the whole point of decoding the sublink array: `Gen 1x2` and
    /// `Gen 2x1` both aggregate to 10 Gbps and are indistinguishable from
    /// sysfs `speed` alone.
    pub fn gen_label(&self) -> Option<String> {
        let lanes = self.effective_lane_count();
        let gen = match self.max_lane_speed_mbps() {
            5_000 => 1,
            10_000 => 2,
            20_000 => 3,
            _ => return None,
        };
        Some(format!("Gen {gen}x{lanes}"))
    }
}

/// One decoded Device Capability descriptor.
///
/// Unknown `bDevCapabilityType` values are preserved verbatim as
/// [`BosCapability::Other`] rather than dropped, so a future capability is
/// still visible to callers (and to `--json` consumers) without a decoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum BosCapability {
    /// `0x02` — USB 2.0 Extension (LPM / BESL).
    Usb2Extension(Usb2Extension),
    /// `0x03` — SuperSpeed USB Device Capability.
    SuperSpeed(SuperSpeedCapability),
    /// `0x04` — Container ID (a 16-byte UUID stable across the device's
    /// interfaces and across re-enumeration on a different port).
    ContainerId {
        /// Raw 16-byte UUID, in descriptor byte order.
        uuid: [u8; 16],
    },
    /// `0x0A` — SuperSpeedPlus USB Device Capability.
    SuperSpeedPlus(SuperSpeedPlusCapability),
    /// Any other `bDevCapabilityType`, kept with its payload so nothing is
    /// silently lost.
    Other {
        /// The raw `bDevCapabilityType` byte.
        cap_type: u8,
        /// Descriptor bytes after the 3-byte
        /// `bLength`/`bDescriptorType`/`bDevCapabilityType` prefix.
        data: Vec<u8>,
    },
}

impl BosCapability {
    /// The `bDevCapabilityType` this capability was decoded from.
    pub fn cap_type(&self) -> u8 {
        match self {
            BosCapability::Usb2Extension(_) => CAP_USB_2_0_EXTENSION,
            BosCapability::SuperSpeed(_) => CAP_SUPERSPEED_USB,
            BosCapability::ContainerId { .. } => CAP_CONTAINER_ID,
            BosCapability::SuperSpeedPlus(_) => CAP_SUPERSPEED_PLUS,
            BosCapability::Other { cap_type, .. } => *cap_type,
        }
    }
}

/// A device's decoded Binary Object Store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BosDescriptors {
    /// `bNumDeviceCaps` as declared by the BOS root descriptor. May exceed
    /// `capabilities.len()` on a truncated blob.
    pub num_device_caps: u8,
    /// `wTotalLength` as declared by the BOS root descriptor.
    pub total_length: u16,
    /// Capabilities decoded, in descriptor order.
    pub capabilities: Vec<BosCapability>,
    /// `true` when the walk stopped early — the blob ran out, a descriptor
    /// claimed a length that would overrun it, or a descriptor declared an
    /// impossible `bLength`. Whatever decoded cleanly is still in
    /// `capabilities`.
    pub truncated: bool,
}

impl BosDescriptors {
    /// The SuperSpeed capability, if the device published one.
    pub fn superspeed(&self) -> Option<&SuperSpeedCapability> {
        self.capabilities.iter().find_map(|c| match c {
            BosCapability::SuperSpeed(s) => Some(s),
            _ => None,
        })
    }

    /// The SuperSpeedPlus capability, if the device published one.
    pub fn superspeed_plus(&self) -> Option<&SuperSpeedPlusCapability> {
        self.capabilities.iter().find_map(|c| match c {
            BosCapability::SuperSpeedPlus(s) => Some(s),
            _ => None,
        })
    }

    /// The USB 2.0 Extension capability, if the device published one.
    pub fn usb2_extension(&self) -> Option<&Usb2Extension> {
        self.capabilities.iter().find_map(|c| match c {
            BosCapability::Usb2Extension(e) => Some(e),
            _ => None,
        })
    }

    /// The Container ID UUID, if the device published one.
    pub fn container_id(&self) -> Option<[u8; 16]> {
        self.capabilities.iter().find_map(|c| match c {
            BosCapability::ContainerId { uuid } => Some(*uuid),
            _ => None,
        })
    }

    /// Container ID rendered as a lowercase hyphenated UUID string, or
    /// `None` when absent or all-zero (some devices publish a zero UUID as
    /// a placeholder, which identifies nothing).
    pub fn container_id_string(&self) -> Option<String> {
        let uuid = self.container_id()?;
        if uuid.iter().all(|&b| b == 0) {
            return None;
        }
        let hex: String = uuid.iter().map(|b| format!("{b:02x}")).collect();
        Some(format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        ))
    }

    /// Highest aggregate data rate the device advertises, in Mbps.
    ///
    /// Takes the larger of the SuperSpeed `wSpeedsSupported` maximum and the
    /// SuperSpeedPlus sublink rollup (`per-lane speed × lane count`). Zero
    /// when neither capability is present — which is the normal case for a
    /// device whose BOS carries only, say, a Billboard capability.
    pub fn max_capable_mbps(&self) -> u32 {
        let ss = self.superspeed().map(|s| s.max_speed_mbps()).unwrap_or(0);
        let ssp = self
            .superspeed_plus()
            .map(|s| s.max_capable_mbps())
            .unwrap_or(0);
        ss.max(ssp)
    }

    /// The vendor's own declared speed floor for full functionality, in
    /// Mbps, or `None` when the device declares none.
    ///
    /// Only the SuperSpeed capability carries a floor this decoder can map
    /// onto a Mbps figure; the SuperSpeedPlus `min_functional_ssid` names a
    /// sublink ID, which is resolved against the sublink array when possible.
    pub fn functional_floor_mbps(&self) -> Option<u32> {
        if let Some(f) = self.superspeed().and_then(|s| s.functional_floor_mbps()) {
            return Some(f);
        }
        let ssp = self.superspeed_plus()?;
        let lanes = ssp.effective_lane_count();
        ssp.sublink_attrs
            .iter()
            .filter(|a| a.ssid == ssp.min_functional_ssid)
            .map(|a| a.lane_speed_mbps().saturating_mul(lanes))
            .max()
    }

    /// USB-IF generation label for the advertised capability
    /// (`"Gen 2x1"`, `"Gen 1x2"`, …), or `None` when not derivable.
    pub fn gen_label(&self) -> Option<String> {
        self.superspeed_plus().and_then(|s| s.gen_label())
    }

    /// Minimum receive / transmit lane counts the device declares, or `None`
    /// when it published no SuperSpeedPlus capability.
    pub fn lane_counts(&self) -> Option<(u8, u8)> {
        self.superspeed_plus()
            .map(|s| (s.min_rx_lane_count, s.min_tx_lane_count))
    }
}

/// How a device's negotiated link speed compares with what its BOS says it
/// can do.
///
/// Deliberately **not** a [`crate::Bottleneck`]: that enum, its
/// [`crate::ChargingDiagnostic`], and the `CapabilityDegraded` D-Bus signal
/// are entirely power-side and keyed on Type-C port numbers. A data-rate
/// shortfall on a plain USB device behind a hub has no port number, so it
/// gets its own vocabulary keyed on the summary `id` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DataRateVerdict {
    /// The link negotiated at (or above) everything the device advertises.
    AtCapability,
    /// The link is slower than the device's advertised maximum, but at or
    /// above the vendor's own declared floor for full functionality — or the
    /// vendor declared no floor at all. Informational: the device may be
    /// perfectly happy here, so this is **never** a warning.
    BelowCapability,
    /// The link negotiated **below the vendor's own declared floor for full
    /// functionality**. The device is not getting what its maker says it
    /// needs — the clear "wrong port / wrong cable" case.
    Degraded,
    /// Not enough information — no BOS, no speed-bearing capability in it,
    /// or the negotiated speed is unknown.
    Unknown,
}

impl DataRateVerdict {
    /// Stable machine string used on the D-Bus `properties` bag and in JSON.
    pub fn label(self) -> &'static str {
        match self {
            DataRateVerdict::AtCapability => "AtCapability",
            DataRateVerdict::BelowCapability => "BelowCapability",
            DataRateVerdict::Degraded => "Degraded",
            DataRateVerdict::Unknown => "Unknown",
        }
    }
}

/// Result of comparing a negotiated link speed against a decoded BOS.
///
/// The data-rate counterpart of [`crate::ChargingDiagnostic`], kept as a
/// separate type on purpose (see [`DataRateVerdict`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataRateAssessment {
    /// Categorical verdict.
    pub verdict: DataRateVerdict,
    /// Speed the link actually negotiated, in Mbps (sysfs `speed`).
    pub negotiated_mbps: u32,
    /// Highest aggregate rate the device advertises, in Mbps. Zero when the
    /// BOS carries no speed-bearing capability.
    pub capable_mbps: u32,
    /// The vendor's declared floor for full functionality, in Mbps. Zero
    /// when the device declares none.
    pub functional_floor_mbps: u32,
    /// USB-IF generation label for the advertised capability, `""` when not
    /// derivable (e.g. no SuperSpeedPlus capability).
    pub capable_gen: String,
    /// One-line headline, English prose.
    pub summary: String,
    /// Optional second-line detail, English prose. `""` when there is
    /// nothing more to say.
    pub detail: String,
    /// `true` only for [`DataRateVerdict::Degraded`]. Conservative by
    /// design: a device that declares full functionality at the speed it
    /// negotiated is never a warning, even when it advertises more.
    pub is_warning: bool,
}

/// Render an Mbps figure the way the CLI and the D-Bus prose do.
fn rate_str(mbps: u32) -> String {
    if mbps >= 1_000 && mbps % 1_000 == 0 {
        format!("{} Gbps", mbps / 1_000)
    } else if mbps >= 1_000 {
        format!("{:.1} Gbps", mbps as f64 / 1_000.0)
    } else {
        format!("{mbps} Mbps")
    }
}

impl DataRateAssessment {
    /// Compare `negotiated_mbps` (sysfs `speed`) against `bos`.
    ///
    /// Returns [`DataRateVerdict::Unknown`] — with empty prose and
    /// `is_warning == false` — whenever either side of the comparison is
    /// missing. Never fabricates a verdict from a partial picture.
    pub fn evaluate(negotiated_mbps: u32, bos: &BosDescriptors) -> DataRateAssessment {
        let capable = bos.max_capable_mbps();
        let floor = bos.functional_floor_mbps();
        let gen = bos.gen_label().unwrap_or_default();

        let mut a = DataRateAssessment {
            verdict: DataRateVerdict::Unknown,
            negotiated_mbps,
            capable_mbps: capable,
            functional_floor_mbps: floor.unwrap_or(0),
            capable_gen: gen,
            summary: String::new(),
            detail: String::new(),
            is_warning: false,
        };

        if capable == 0 || negotiated_mbps == 0 {
            return a;
        }

        if negotiated_mbps >= capable {
            a.verdict = DataRateVerdict::AtCapability;
            a.summary = format!("Running at its full {} capability", rate_str(capable));
            return a;
        }

        // Below the advertised maximum. Whether that is a *problem* is the
        // vendor's call, via bFunctionalitySupport: a device that declares
        // full functionality at the speed it got is fine where it is, even
        // though it can go faster elsewhere.
        match floor {
            Some(f) if negotiated_mbps < f => {
                a.verdict = DataRateVerdict::Degraded;
                a.is_warning = true;
                a.summary = format!(
                    "Linked at {} — below the {} this device needs",
                    rate_str(negotiated_mbps),
                    rate_str(f)
                );
                a.detail = format!(
                    "Device advertises up to {}{}; move it to a faster port or use a cable that supports it",
                    rate_str(capable),
                    if a.capable_gen.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", a.capable_gen)
                    }
                );
            }
            _ => {
                a.verdict = DataRateVerdict::BelowCapability;
                a.summary = format!(
                    "Linked at {} of an advertised {}",
                    rate_str(negotiated_mbps),
                    rate_str(capable)
                );
                a.detail = match floor {
                    Some(f) => format!(
                        "Vendor declares full functionality from {} — not a fault",
                        rate_str(f)
                    ),
                    None => "Device declares no minimum speed for full functionality".to_string(),
                };
            }
        }
        a
    }
}

/// Decode a raw `bos_descriptors` blob.
///
/// Returns `None` when `blob` is not a BOS at all — too short, or a root
/// descriptor with the wrong `bLength` / `bDescriptorType`. A blob that
/// *starts* as a valid BOS but is cut short or holds a malformed capability
/// yields `Some` with [`BosDescriptors::truncated`] set and whatever decoded
/// cleanly.
pub fn parse(blob: &[u8]) -> Option<BosDescriptors> {
    if blob.len() < BOS_HEADER_LEN {
        return None;
    }
    if blob[0] as usize != BOS_HEADER_LEN || blob[1] != DT_BOS {
        return None;
    }

    let total_length = u16::from_le_bytes([blob[2], blob[3]]);
    let num_device_caps = blob[4];

    // Trust the blob's actual length over the declared wTotalLength; a
    // declared length longer than what we were handed is exactly the
    // truncation case.
    let declared_end = total_length as usize;
    let end = declared_end.min(blob.len());
    let truncated_header = declared_end > blob.len();

    let mut out = BosDescriptors {
        num_device_caps,
        total_length,
        capabilities: Vec::new(),
        truncated: truncated_header,
    };

    let mut off = BOS_HEADER_LEN;
    while off + 3 <= end {
        let len = blob[off] as usize;
        // A descriptor must carry at least bLength + bDescriptorType +
        // bDevCapabilityType. Anything shorter (0 included) would not
        // advance the walk — bail rather than loop.
        if len < 3 {
            out.truncated = true;
            break;
        }
        if off + len > end {
            out.truncated = true;
            break;
        }
        if blob[off + 1] != DT_DEVICE_CAPABILITY {
            // Not a Device Capability descriptor — the blob is not laid out
            // the way the spec says. Stop rather than guess.
            out.truncated = true;
            break;
        }
        if out.capabilities.len() >= MAX_CAPABILITIES {
            out.truncated = true;
            break;
        }

        let cap_type = blob[off + 2];
        let body = &blob[off + 3..off + len];
        out.capabilities.push(decode_capability(cap_type, body));
        off += len;
    }

    if off < end {
        out.truncated = true;
    }

    Some(out)
}

/// Decode one capability body (everything after the 3-byte prefix).
///
/// A body too short for its declared type falls back to
/// [`BosCapability::Other`] so the bytes survive.
fn decode_capability(cap_type: u8, body: &[u8]) -> BosCapability {
    match cap_type {
        CAP_USB_2_0_EXTENSION if body.len() >= 4 => {
            let bits = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
            BosCapability::Usb2Extension(Usb2Extension::from_bits(bits))
        }
        CAP_SUPERSPEED_USB if body.len() >= 7 => BosCapability::SuperSpeed(SuperSpeedCapability {
            ltm_capable: body[0] & (1 << 1) != 0,
            speeds_supported: u16::from_le_bytes([body[1], body[2]]),
            functionality_support: body[3],
            u1_dev_exit_lat_us: body[4],
            u2_dev_exit_lat_us: u16::from_le_bytes([body[5], body[6]]),
        }),
        CAP_CONTAINER_ID if body.len() >= 17 => {
            // body[0] is bReserved; the UUID follows.
            let mut uuid = [0u8; 16];
            uuid.copy_from_slice(&body[1..17]);
            BosCapability::ContainerId { uuid }
        }
        CAP_SUPERSPEED_PLUS if body.len() >= 9 => {
            // body: bReserved, bmAttributes[4], wFunctionalitySupport[2],
            //       wReserved[2], then bmSublinkSpeedAttr[] (4 bytes each).
            let bm = u32::from_le_bytes([body[1], body[2], body[3], body[4]]);
            let func = u16::from_le_bytes([body[5], body[6]]);
            let attr_count = ((bm & 0x1F) as u8).saturating_add(1);
            let id_count = (((bm >> 5) & 0xF) as u8).saturating_add(1);

            let attrs_body = &body[9..];
            let available = attrs_body.len() / 4;
            let take = (attr_count as usize).min(available);
            let sublink_attrs = (0..take)
                .map(|i| {
                    let o = i * 4;
                    SublinkSpeedAttribute::from_bits(u32::from_le_bytes([
                        attrs_body[o],
                        attrs_body[o + 1],
                        attrs_body[o + 2],
                        attrs_body[o + 3],
                    ]))
                })
                .collect();

            BosCapability::SuperSpeedPlus(SuperSpeedPlusCapability {
                sublink_speed_attr_count: attr_count,
                sublink_speed_id_count: id_count,
                min_functional_ssid: (func & 0xF) as u8,
                min_rx_lane_count: ((func >> 8) & 0xF) as u8,
                min_tx_lane_count: ((func >> 12) & 0xF) as u8,
                sublink_attrs,
            })
        }
        other => BosCapability::Other {
            cap_type: other,
            data: body.to_vec(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real blob — "USB 10/100/1000 LAN" at `5-2.1.1`, negotiated 480 Mbps
    /// behind a USB 2.0 hub.
    const LAN_5_2_1_1: &[u8] = &[
        0x05, 0x0f, 0x16, 0x00, 0x02, // BOS header, 2 caps, wTotalLength 0x16
        0x07, 0x10, 0x02, 0x06, 0x00, 0x00, 0x00, // USB 2.0 Extension
        0x0a, 0x10, 0x03, 0x02, 0x0e, 0x00, 0x02, 0x0a, 0xff, 0x07, // SuperSpeed
    ];

    /// Real blob — "4-Port USB 2.0 Hub" at `5-2`.
    const HUB_5_2: &[u8] = &[
        0x05, 0x0f, 0x2a, 0x00, 0x03, // BOS header, 3 caps
        0x07, 0x10, 0x02, 0x1e, 0xf4, 0x00, 0x00, // USB 2.0 Extension
        0x0a, 0x10, 0x03, 0x00, 0x0e, 0x00, 0x01, 0x0a, 0xff, 0x03, // SuperSpeed
        0x14, 0x10, 0x04, 0x00, // Container ID header + bReserved
        0xf1, 0xad, 0xf5, 0xec, 0x11, 0x50, 0x05, 0x40, 0x91, 0xec, 0x71, 0xca, 0x71, 0x01, 0xb6,
        0xa2,
    ];

    /// Real blob — xHCI root hub `usb2`, SuperSpeedPlus 10 Gbps single lane.
    const ROOT_HUB_USB2: &[u8] = &[
        0x05, 0x0f, 0x2b, 0x00, 0x02, // BOS header, 2 caps
        0x0a, 0x10, 0x03, 0x02, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, // SuperSpeed
        0x1c, 0x10, 0x0a, 0x00, // SSP header + bReserved
        0x23, 0x00, 0x00, 0x00, // bmAttributes: SSAC=3 (4 attrs), SSIC=1
        0x04, 0x11, // wFunctionalitySupport: ssid 4, rx 1, tx 1
        0x00, 0x00, // wReserved
        0x34, 0x00, 0x05, 0x00, // ssid 4, 5 Gbps Rx, SuperSpeed
        0xb4, 0x00, 0x05, 0x00, // ssid 4, 5 Gbps Tx, SuperSpeed
        0x35, 0x40, 0x0a, 0x00, // ssid 5, 10 Gbps Rx, SuperSpeedPlus
        0xb5, 0x40, 0x0a, 0x00, // ssid 5, 10 Gbps Tx, SuperSpeedPlus
    ];

    /// Real blob — Full-Speed-only device at `1-4`, USB 2.0 Extension alone.
    const FS_ONLY_1_4: &[u8] = &[
        0x05, 0x0f, 0x0c, 0x00, 0x01, // BOS header, 1 cap
        0x07, 0x10, 0x02, 0x0e, 0x04, 0x00, 0x00, // USB 2.0 Extension
    ];

    #[test]
    fn parses_real_lan_adapter_blob() {
        let b = parse(LAN_5_2_1_1).expect("valid BOS");
        assert!(!b.truncated);
        assert_eq!(b.num_device_caps, 2);
        assert_eq!(b.total_length, 0x16);
        assert_eq!(b.capabilities.len(), 2);

        let ext = b.usb2_extension().expect("USB 2.0 Extension");
        assert_eq!(ext.bm_attributes, 0x0000_0006);
        assert!(ext.lpm);
        assert!(ext.besl);
        assert!(!ext.baseline_besl_valid);

        let ss = b.superspeed().expect("SuperSpeed cap");
        assert!(ss.ltm_capable);
        assert_eq!(ss.speeds_supported, 0x000E);
        assert!(!ss.supports_low());
        assert!(ss.supports_full());
        assert!(ss.supports_high());
        assert!(ss.supports_gen1());
        assert_eq!(ss.functionality_support, 2);
        assert_eq!(ss.u1_dev_exit_lat_us, 0x0a);
        assert_eq!(ss.u2_dev_exit_lat_us, 0x07ff);

        assert_eq!(b.max_capable_mbps(), 5_000);
        assert_eq!(b.functional_floor_mbps(), Some(480));
        assert_eq!(b.gen_label(), None);
        assert_eq!(b.container_id(), None);
    }

    #[test]
    fn parses_real_hub_blob_with_container_id() {
        let b = parse(HUB_5_2).expect("valid BOS");
        assert!(!b.truncated);
        assert_eq!(b.num_device_caps, 3);
        assert_eq!(b.capabilities.len(), 3);

        let ext = b.usb2_extension().unwrap();
        assert_eq!(ext.bm_attributes, 0x0000_f41e);
        assert!(ext.lpm && ext.besl && ext.baseline_besl_valid && ext.deep_besl_valid);
        assert_eq!(ext.baseline_besl, 4);
        assert_eq!(ext.deep_besl, 15);

        let ss = b.superspeed().unwrap();
        assert!(!ss.ltm_capable);
        assert_eq!(ss.speeds_supported, 0x000E);
        assert_eq!(ss.functionality_support, 1);

        assert_eq!(b.max_capable_mbps(), 5_000);
        // Vendor says full functionality already at Full Speed.
        assert_eq!(b.functional_floor_mbps(), Some(12));

        let uuid = b.container_id().expect("container id");
        assert_eq!(uuid[0], 0xf1);
        assert_eq!(uuid[15], 0xa2);
        assert_eq!(
            b.container_id_string().as_deref(),
            Some("f1adf5ec-1150-0540-91ec-71ca7101b6a2")
        );
    }

    #[test]
    fn parses_real_root_hub_superspeed_plus_blob() {
        let b = parse(ROOT_HUB_USB2).expect("valid BOS");
        assert!(!b.truncated);
        assert_eq!(b.capabilities.len(), 2);

        let ssp = b.superspeed_plus().expect("SSP cap");
        assert_eq!(ssp.sublink_speed_attr_count, 4);
        assert_eq!(ssp.sublink_speed_id_count, 2);
        assert_eq!(ssp.min_functional_ssid, 4);
        assert_eq!(ssp.min_rx_lane_count, 1);
        assert_eq!(ssp.min_tx_lane_count, 1);
        assert_eq!(ssp.sublink_attrs.len(), 4);

        let a0 = ssp.sublink_attrs[0];
        assert_eq!(a0.ssid, 4);
        assert_eq!(a0.exponent, LaneSpeedExponent::Gbps);
        assert_eq!(a0.sublink_type, SublinkType::SymmetricRx);
        assert!(a0.sublink_type.is_rx());
        assert_eq!(a0.protocol, LinkProtocol::SuperSpeed);
        assert_eq!(a0.mantissa, 5);
        assert_eq!(a0.lane_speed_mbps(), 5_000);

        let a1 = ssp.sublink_attrs[1];
        assert_eq!(a1.sublink_type, SublinkType::SymmetricTx);
        assert!(a1.sublink_type.is_tx());

        let a3 = ssp.sublink_attrs[3];
        assert_eq!(a3.ssid, 5);
        assert_eq!(a3.protocol, LinkProtocol::SuperSpeedPlus);
        assert_eq!(a3.lane_speed_mbps(), 10_000);
        assert_eq!(a3.sublink_type, SublinkType::SymmetricTx);

        assert_eq!(ssp.max_lane_speed_mbps(), 10_000);
        assert_eq!(ssp.effective_lane_count(), 1);
        assert_eq!(b.max_capable_mbps(), 10_000);
        assert_eq!(b.gen_label().as_deref(), Some("Gen 2x1"));
        // SuperSpeed cap declares bit 3 (Gen 1) as the functionality floor.
        assert_eq!(b.functional_floor_mbps(), Some(5_000));
    }

    #[test]
    fn parses_real_full_speed_only_blob() {
        let b = parse(FS_ONLY_1_4).expect("valid BOS");
        assert!(!b.truncated);
        assert_eq!(b.capabilities.len(), 1);
        let ext = b.usb2_extension().unwrap();
        assert_eq!(ext.bm_attributes, 0x0000_040e);
        assert_eq!(ext.baseline_besl, 4);
        assert!(ext.baseline_besl_valid);
        assert!(!ext.deep_besl_valid);
        // No speed-bearing capability at all.
        assert_eq!(b.max_capable_mbps(), 0);
        assert_eq!(b.functional_floor_mbps(), None);
    }

    #[test]
    fn gen1x2_and_gen2x1_are_distinguishable() {
        // Both aggregate to 10 Gbps and both report speed == 10000 in sysfs.
        let gen2x1 = SuperSpeedPlusCapability {
            sublink_speed_attr_count: 2,
            sublink_speed_id_count: 1,
            min_functional_ssid: 0,
            min_rx_lane_count: 1,
            min_tx_lane_count: 1,
            sublink_attrs: vec![
                SublinkSpeedAttribute::from_bits(0x000A_4030),
                SublinkSpeedAttribute::from_bits(0x000A_40B0),
            ],
        };
        let gen1x2 = SuperSpeedPlusCapability {
            sublink_speed_attr_count: 2,
            sublink_speed_id_count: 1,
            min_functional_ssid: 0,
            min_rx_lane_count: 2,
            min_tx_lane_count: 2,
            sublink_attrs: vec![
                SublinkSpeedAttribute::from_bits(0x0005_0030),
                SublinkSpeedAttribute::from_bits(0x0005_00b0),
            ],
        };
        assert_eq!(gen2x1.max_capable_mbps(), 10_000);
        assert_eq!(gen1x2.max_capable_mbps(), 10_000);
        assert_eq!(gen2x1.gen_label().as_deref(), Some("Gen 2x1"));
        assert_eq!(gen1x2.gen_label().as_deref(), Some("Gen 1x2"));
    }

    #[test]
    fn lane_speed_exponents_scale() {
        // mantissa 10, Gbps
        assert_eq!(
            SublinkSpeedAttribute::from_bits(0x000A_0030).lane_speed_mbps(),
            10_000
        );
        // mantissa 480, Mbps (exponent bits = 2)
        assert_eq!(
            SublinkSpeedAttribute::from_bits(0x01E0_0020).lane_speed_mbps(),
            480
        );
        // mantissa 5000, Kbps (exponent bits = 1)
        assert_eq!(
            SublinkSpeedAttribute::from_bits(0x1388_0010).lane_speed_mbps(),
            5
        );
        // mantissa 65535, bps (exponent bits = 0) → sub-Mbps, floors to 0
        assert_eq!(
            SublinkSpeedAttribute::from_bits(0xFFFF_0000).lane_speed_mbps(),
            0
        );
        // Absurd Gbps mantissa saturates instead of overflowing.
        assert_eq!(
            SublinkSpeedAttribute::from_bits(0xFFFF_0030).lane_speed_mbps(),
            65_535_000
        );
    }

    // ---- malformed / hostile inputs -------------------------------------

    #[test]
    fn empty_blob_is_not_a_bos() {
        assert!(parse(&[]).is_none());
        assert!(parse(&[0x05]).is_none());
        assert!(parse(&[0x05, 0x0f, 0x16, 0x00]).is_none());
    }

    #[test]
    fn wrong_descriptor_type_is_rejected() {
        assert!(parse(&[0x05, 0x01, 0x05, 0x00, 0x00]).is_none());
    }

    #[test]
    fn wrong_header_length_is_rejected() {
        assert!(parse(&[0x04, 0x0f, 0x05, 0x00, 0x00]).is_none());
        assert!(parse(&[0x00, 0x0f, 0x05, 0x00, 0x00]).is_none());
    }

    #[test]
    fn header_only_blob_parses_with_no_capabilities() {
        let b = parse(&[0x05, 0x0f, 0x05, 0x00, 0x00]).expect("valid header");
        assert!(b.capabilities.is_empty());
        assert!(!b.truncated);
        assert_eq!(b.max_capable_mbps(), 0);
        assert_eq!(b.functional_floor_mbps(), None);
    }

    #[test]
    fn zero_blength_capability_terminates_the_walk() {
        // A zero bLength would advance the offset by nothing — must not loop.
        let blob = [
            0x05, 0x0f, 0x0c, 0x00, 0x02, // header claims 2 caps
            0x00, 0x10, 0x02, 0x00, 0x00, 0x00, 0x00, // bLength == 0
        ];
        let b = parse(&blob).expect("valid header");
        assert!(b.truncated);
        assert!(b.capabilities.is_empty());
    }

    #[test]
    fn short_blength_capability_terminates_the_walk() {
        let blob = [
            0x05, 0x0f, 0x0c, 0x00, 0x02, //
            0x02, 0x10, 0x02, 0x00, 0x00, 0x00, 0x00, // bLength == 2 (< 3)
        ];
        let b = parse(&blob).expect("valid header");
        assert!(b.truncated);
        assert!(b.capabilities.is_empty());
    }

    #[test]
    fn capability_overrunning_the_blob_is_truncated() {
        // Declares a 10-byte SuperSpeed cap but only 5 bytes follow.
        let blob = [
            0x05, 0x0f, 0x0f, 0x00, 0x01, //
            0x0a, 0x10, 0x03, 0x02, 0x0e,
        ];
        let b = parse(&blob).expect("valid header");
        assert!(b.truncated);
        assert!(b.capabilities.is_empty());
        assert_eq!(b.max_capable_mbps(), 0);
    }

    #[test]
    fn blob_shorter_than_declared_total_length_keeps_what_parsed() {
        // wTotalLength claims 0x16 but only the first capability is present.
        let mut blob = LAN_5_2_1_1.to_vec();
        blob.truncate(12);
        let b = parse(&blob).expect("valid header");
        assert!(b.truncated);
        assert_eq!(b.capabilities.len(), 1);
        assert!(b.usb2_extension().is_some());
        assert_eq!(b.max_capable_mbps(), 0);
    }

    #[test]
    fn non_device_capability_descriptor_type_stops_the_walk() {
        let blob = [
            0x05, 0x0f, 0x0c, 0x00, 0x01, //
            0x07, 0x21, 0x02, 0x06, 0x00, 0x00, 0x00, // bDescriptorType 0x21
        ];
        let b = parse(&blob).expect("valid header");
        assert!(b.truncated);
        assert!(b.capabilities.is_empty());
    }

    #[test]
    fn unknown_capability_type_is_preserved_verbatim() {
        let blob = [
            0x05, 0x0f, 0x0b, 0x00, 0x01, //
            0x06, 0x10, 0x0d, 0xde, 0xad, 0xbe, // Billboard (0x0d)
        ];
        let b = parse(&blob).expect("valid header");
        assert!(!b.truncated);
        assert_eq!(b.capabilities.len(), 1);
        match &b.capabilities[0] {
            BosCapability::Other { cap_type, data } => {
                assert_eq!(*cap_type, 0x0d);
                assert_eq!(data, &[0xde, 0xad, 0xbe]);
            }
            other => panic!("expected Other, got {other:?}"),
        }
        assert_eq!(b.capabilities[0].cap_type(), 0x0d);
    }

    #[test]
    fn undersized_typed_capability_falls_back_to_other() {
        // SuperSpeed cap declared but only 4 body bytes (needs 7).
        let blob = [
            0x05, 0x0f, 0x0c, 0x00, 0x01, //
            0x07, 0x10, 0x03, 0x02, 0x0e, 0x00, 0x02,
        ];
        let b = parse(&blob).expect("valid header");
        assert!(b.superspeed().is_none());
        assert_eq!(b.capabilities[0].cap_type(), CAP_SUPERSPEED_USB);
        assert!(matches!(
            b.capabilities[0],
            BosCapability::Other { cap_type: 0x03, .. }
        ));
    }

    #[test]
    fn superspeed_plus_with_truncated_attr_array_keeps_whole_entries() {
        // Claims 4 attributes, supplies 1 and a half.
        let blob = [
            0x05, 0x0f, 0x18, 0x00, 0x01, //
            0x13, 0x10, 0x0a, 0x00, // SSP, bLength 0x13 = 19
            0x23, 0x00, 0x00, 0x00, // SSAC=3 → 4 attrs claimed
            0x04, 0x11, 0x00, 0x00, //
            0x34, 0x00, 0x05, 0x00, // one whole attribute
            0xb4, 0x00, 0x05, // three quarters of the next
        ];
        let b = parse(&blob).expect("valid header");
        let ssp = b.superspeed_plus().expect("SSP");
        assert_eq!(ssp.sublink_speed_attr_count, 4);
        assert_eq!(ssp.sublink_attrs.len(), 1);
        assert_eq!(b.max_capable_mbps(), 5_000);
    }

    #[test]
    fn contradictory_functionality_support_is_not_trusted() {
        // bFunctionalitySupport names bit 3 but bit 3 is not set.
        let ss = SuperSpeedCapability {
            ltm_capable: false,
            speeds_supported: 0x0006, // full + high only
            functionality_support: 3, // claims Gen 1
            u1_dev_exit_lat_us: 0,
            u2_dev_exit_lat_us: 0,
        };
        assert_eq!(ss.functional_floor_mbps(), None);
        assert_eq!(ss.max_speed_mbps(), 480);

        // Out-of-range index is also rejected.
        let ss = SuperSpeedCapability {
            functionality_support: 9,
            speeds_supported: 0xFFFF,
            ..ss
        };
        assert_eq!(ss.functional_floor_mbps(), None);
    }

    #[test]
    fn zero_container_id_is_reported_as_absent() {
        let mut blob = vec![0x05, 0x0f, 0x19, 0x00, 0x01, 0x14, 0x10, 0x04, 0x00];
        blob.extend_from_slice(&[0u8; 16]);
        let b = parse(&blob).expect("valid header");
        assert_eq!(b.container_id(), Some([0u8; 16]));
        assert_eq!(b.container_id_string(), None);
    }

    // ---- assessment -----------------------------------------------------

    #[test]
    fn assessment_suppresses_warning_at_vendor_declared_floor() {
        // The headline false-positive case: the LAN adapter advertises
        // SuperSpeed but declares full functionality at High Speed, and it
        // is linked at High Speed. Informational, never a warning.
        let b = parse(LAN_5_2_1_1).unwrap();
        let a = DataRateAssessment::evaluate(480, &b);
        assert_eq!(a.verdict, DataRateVerdict::BelowCapability);
        assert!(!a.is_warning);
        assert_eq!(a.negotiated_mbps, 480);
        assert_eq!(a.capable_mbps, 5_000);
        assert_eq!(a.functional_floor_mbps, 480);
        assert!(a.summary.contains("480 Mbps"));
        assert!(a.detail.contains("not a fault"));
    }

    #[test]
    fn assessment_flags_below_the_declared_floor() {
        // Same device forced onto a Full-Speed link: now below its own floor.
        let b = parse(LAN_5_2_1_1).unwrap();
        let a = DataRateAssessment::evaluate(12, &b);
        assert_eq!(a.verdict, DataRateVerdict::Degraded);
        assert!(a.is_warning);
        assert!(a.summary.contains("12 Mbps"));
        assert!(a.detail.contains("5 Gbps"));
    }

    #[test]
    fn assessment_at_capability_is_never_a_warning() {
        let b = parse(LAN_5_2_1_1).unwrap();
        let a = DataRateAssessment::evaluate(5_000, &b);
        assert_eq!(a.verdict, DataRateVerdict::AtCapability);
        assert!(!a.is_warning);
        assert!(a.detail.is_empty());

        // Negotiated *above* the advertised max is still AtCapability.
        let a = DataRateAssessment::evaluate(10_000, &b);
        assert_eq!(a.verdict, DataRateVerdict::AtCapability);
        assert!(!a.is_warning);
    }

    #[test]
    fn assessment_is_unknown_without_usable_inputs() {
        let b = parse(FS_ONLY_1_4).unwrap();
        // No speed-bearing capability.
        let a = DataRateAssessment::evaluate(12, &b);
        assert_eq!(a.verdict, DataRateVerdict::Unknown);
        assert!(!a.is_warning);
        assert!(a.summary.is_empty());

        // Unknown negotiated speed.
        let b = parse(LAN_5_2_1_1).unwrap();
        let a = DataRateAssessment::evaluate(0, &b);
        assert_eq!(a.verdict, DataRateVerdict::Unknown);
        assert!(!a.is_warning);
    }

    #[test]
    fn assessment_without_declared_floor_is_never_a_warning() {
        // SuperSpeed-capable device that declares no usable floor.
        let blob = [
            0x05, 0x0f, 0x0f, 0x00, 0x01, //
            0x0a, 0x10, 0x03, 0x00, 0x08, 0x00, 0x09, 0x00, 0x00, 0x00,
        ];
        let b = parse(&blob).unwrap();
        assert_eq!(b.max_capable_mbps(), 5_000);
        assert_eq!(b.functional_floor_mbps(), None);
        let a = DataRateAssessment::evaluate(480, &b);
        assert_eq!(a.verdict, DataRateVerdict::BelowCapability);
        assert!(!a.is_warning);
        assert_eq!(a.functional_floor_mbps, 0);
        assert!(a.detail.contains("no minimum speed"));
    }

    #[test]
    fn assessment_carries_gen_label_when_derivable() {
        let b = parse(ROOT_HUB_USB2).unwrap();
        let a = DataRateAssessment::evaluate(10_000, &b);
        assert_eq!(a.capable_gen, "Gen 2x1");
        assert_eq!(a.verdict, DataRateVerdict::AtCapability);
    }

    #[test]
    fn rate_str_formats_tiers() {
        assert_eq!(rate_str(480), "480 Mbps");
        assert_eq!(rate_str(12), "12 Mbps");
        assert_eq!(rate_str(5_000), "5 Gbps");
        assert_eq!(rate_str(10_000), "10 Gbps");
        assert_eq!(rate_str(1_500), "1.5 Gbps");
    }

    #[test]
    fn verdict_labels_are_stable() {
        assert_eq!(DataRateVerdict::AtCapability.label(), "AtCapability");
        assert_eq!(DataRateVerdict::BelowCapability.label(), "BelowCapability");
        assert_eq!(DataRateVerdict::Degraded.label(), "Degraded");
        assert_eq!(DataRateVerdict::Unknown.label(), "Unknown");
    }
}
