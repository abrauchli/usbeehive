---
title: Devices3 wire — trust signals, transport flags, PDO list, phone classifier
status: in-progress
date: 2026-05-26
version_bump: 0.6.0 → 0.7.0 (breaking — interface rename)
interface_bump: org.usbeehive.Devices2 → org.usbeehive.Devices3
---

# Quick Task: Devices3 v3 wire — four enhancements + breaking bump

## Motivation

UI inquiry from the usbee GNOME extension surfaced three gaps in the Devices2
wire (trust-signals, PDO list, active-transport flags) and one backend
misclassification (Cat S61 / Android composite phones rendered with a serial
console icon). User opted to break Devices2 — UI side hasn't shipped, so it's
the right time. Cut to **Devices3**, bump crate to **0.7.0**.

## Scope (5 atomic chunks)

### Task 1 — Phone classifier fix (summary.rs)
- `is_phone(vendor_id, product_lower, ifaces)` gains:
  - ADB function signature `class=0xFF / sub=0x42 / proto=0x01` (bullet-proof).
  - PTP `0x06/0x01/0x01` AND MTP-shaped function together.
  - Phone-VID allowlist + has-PTP-or-MTP fallback.
- New `PHONE_VIDS` const slice: 0x18D1 (Google), 0x04E8 (Samsung), 0x22B8
  (Motorola), 0x0FCE (Sony), 0x1BBB (Bullitt/Cat), 0x2A70 (OnePlus), 0x2717
  (Xiaomi), 0x12D1 (Huawei), 0x0B05 (ASUS), 0x2717, 0x19D2 (ZTE), 0x04E9 (LG),
  0x0BB4 (HTC), 0x1004 (LGE).
- Update call site at `summary.rs:328` to pass `&dev.interfaces`.
- Tests: Cat S61 case (vid=0x1BBB + CDC-ACM iface), Pixel-like (ADB),
  vanilla DSLR (PTP-only) must NOT classify as phone.

**Files**: `src/summary.rs`

### Task 2 — Cable trust signals
- New `CableTrust` struct in `cable.rs` with three bool flags:
  `zero_vid`, `vid_unknown`, `reserved_bits_set`.
- Decoder helper `pd::cable_vdo_reserved_bits_set(raw: u32, is_active: bool) -> bool`:
  passive cables — reserved mask `0x00000088` (bit 3, bits 7-8); active cables —
  reserved mask `0x00000008` (bit 3 only, since 7-8 are SBU-supported).
  Conservative: only check bits with no defined meaning in PD R3.x.
- `CableInfo` gains `trust: CableTrust` field, populated in
  `from_typec_cable`.
- `summary.rs` emits three properties when present:
  `cable.trust.zero_vid=true`, `cable.trust.vid_unknown=true`,
  `cable.trust.reserved_bits=true`. Only push the keys that fire (omit on
  clean cables).
- Tests: zero-VID cable, hex-fallback vendor, garbage VDO with bit 3 set.

**Files**: `src/cable.rs`, `src/pd.rs`, `src/summary.rs`

### Task 3 — Active-transport flags
- Sysfs read: enumerate altmode subdirs `port{N}-partner.M/` under each
  Type-C port, read `svid` (hex) + `mode` + `active`. Add field
  `TypeCPartner.altmodes: Vec<TypeCAltMode>`.
- New `TypeCAltMode { svid: u16, mode: u32, active: bool }` in `typec.rs`.
- `summary.rs` derives four booleans:
  - `transport.usb2` — link_speed_mbps in 1..=480 OR usb_version == "2.0".
  - `transport.usb3` — link_speed_mbps >= 5000.
  - `transport.dp_altmode` — partner has altmode with `svid == 0xFF01`.
  - `transport.tb` — partner has altmode with `svid == 0x8087` OR usb_version starts with "4".
- Properties pushed only when `true` (avoid bag bloat).
- Tests: synthetic partner with DP altmode, partner with TBT altmode,
  pure USB3 device.

**Files**: `src/typec.rs`, `src/sysfs/typec.rs`, `src/summary.rs`

### Task 4 — Structured PDO list on wire
- New `PdoEntry` wire type in `dbus.rs`:
  ```rust
  pub struct PdoEntry {
      pub index: u32,         // 1-based PDO index
      pub kind: String,       // "Fixed" | "Battery" | "Variable" | "PPS"
      pub voltage_mv: u32,    // or min voltage for PPS
      pub max_voltage_mv: u32,// 0 unless PPS/Variable
      pub current_ma: u32,
      pub power_mw: u32,
      pub is_active: bool,
  }
  ```
- `DeviceEntry` gains:
  - `pdo_list: Vec<PdoEntry>` — source caps from `PowerDeliveryPort`.
  - `active_pdo_index: i32` — `-1` when no active contract.
- Populate from `s.power_delivery.source_capabilities` in
  `DeviceEntry::from(&DeviceSummary)`. Empty Vec when no PD port.
- Existing `charger_max` property STAYS (back-compat hint for stringly
  consumers), but PDO list is the structured source of truth.

**Files**: `src/dbus.rs`

### Task 5 — Interface rename + version bump + docs
- `src/dbus.rs`: every `"org.usbeehive.Devices2"` → `"Devices3"` in:
  - module docs (lines 10, 71)
  - `#[interface(name = ...)]` attr (line 306)
  - `///` doc on `DevicesIface` (line 274)
  - Wire signature table — add `pdo_list`, `active_pdo_index` rows and
    update the `ListDevices` signature at line 55.
- `src/bin/usbeehived.rs`: gdbus example comment (lines 13-15).
- `examples/dbus_client.rs:23`: proxy interface arg.
- `tests/dbus_interface.rs`: comment at line 71.
- `Cargo.toml`: `version = "0.6.0"` → `"0.7.0"`. Also feature comment at
  line 50.
- `README.md`: lines 102, 169, 188-190, 195, 197 (Devices2 → Devices3).
- `AGENTS.md`: lines 61, 129 (already lagged on Devices1 — bring to Devices3).
- `CHANGELOG.md`: new entry at top describing the 0.7.0 break.

**Files**: `Cargo.toml`, `CHANGELOG.md`, `README.md`, `AGENTS.md`,
`src/dbus.rs`, `src/bin/usbeehived.rs`, `examples/dbus_client.rs`,
`tests/dbus_interface.rs`

## Commit strategy

One atomic commit per task. Final docs commit gathers PLAN.md + SUMMARY.md
+ CHANGELOG (CHANGELOG ships with task 5 since it's part of the bump).

## Verification

- `cargo build --all-features` clean.
- `cargo test --all-features` passes including new unit tests.
- `cargo clippy --all-features -- -D warnings` clean.
- `grep -r "Devices2" src/ tests/ examples/ README.md AGENTS.md` returns
  zero matches (except CHANGELOG history).
