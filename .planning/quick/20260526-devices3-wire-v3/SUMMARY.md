---
title: Devices3 wire — trust signals, transport flags, PDO list, phone classifier
status: complete
date: 2026-05-26
version: 0.6.0 → 0.7.0
interface: org.usbeehive.Devices2 → org.usbeehive.Devices3
---

# Quick Task: Devices3 v3 wire — completed

Coordinated breaking bump driven by the USBee GNOME extension UI inquiry.
Four enhancements landed atomically with one breaking interface bump and a
matching crate version bump. 150 tests pass, clippy clean across all
features.

## Commits

| Commit | Subject |
|---|---|
| `4ad7853` | Phone classifier — detect Android composites by ADB/PTP+vendor signatures and VID allowlist |
| `d4c7eb2` | Cable trust signals — zero VID, unknown VID, reserved-bits-set heuristics |
| `4db4350` | Active-transport flags — USB2/USB3 from speed, DP/TB altmodes from partner |
| `337830c` | PDO list — structured wire field with active index |
| `8dfb735` | Release 0.7.0 — Devices3 wire (breaking) |

## Wire surface diff

### New per-entry fields (positions 20, 21)

| Pos | Field | Type | Notes |
|---|---|---|---|
| 20 | `pdo_list` | `a(usuuuub)` | Per source PDO: (index, kind, voltage_mv, max_voltage_mv, current_ma, power_mw, is_active). Empty when no PD port. |
| 21 | `active_pdo_index` | `i` | Index into pdo_list, or `-1`. |

Full signature: `a(ssssssssssqqsa(ss)ius(uus)(bsssb)a(usuuuub)i)`.

### New machine-keyed properties (a(ss) bag)

All pushed only when their flag fires.

| Key | Source | Meaning |
|---|---|---|
| `cable.trust.zero_vid` | `CableInfo.trust` | Cable ID Header VDO reports vendor_id == 0. |
| `cable.trust.vid_unknown` | `CableInfo.trust` | Non-zero VID not in USB-IF DB. |
| `cable.trust.reserved_bits` | `CableInfo.trust` | Cable VDO sets PD R3.x-reserved bits. |
| `transport.usb2` | `UsbDevice.speed` | Speed in 1..=480 Mbps. |
| `transport.usb3` | `UsbDevice.speed` | Speed ≥ 5000 Mbps. |
| `transport.dp_altmode` | `TypeCPartner.altmodes` | Partner advertises SVID 0xFF01. |
| `transport.tb` | `TypeCPartner.altmodes` | Partner advertises SVID 0x8087. |

### New library types

- `dbus::PdoEntry` — wire mirror of `power::PowerDataObject`.
- `cable::CableTrust` — zero_vid / vid_unknown / reserved_bits_set booleans.
- `typec::TypeCAltMode` — (svid, mode, active).
- `typec::TypeCPartner.altmodes: Vec<TypeCAltMode>` — populated from
  sibling altmode directories under `/sys/class/typec/`.
- `pd::cable_vdo_reserved_bits_set(vdo, is_active)` — conservative
  reserved-bit mask helper.

### Phone classifier — Cat S61 fix

`summary::is_phone()` signature now takes an `&[UsbInterface]` slice. Three
new signals stacked beneath the existing iPhone + product-string checks:
1. ADB function signature 0xFF/0x42/0x01 — bullet-proof.
2. PTP 0x06/0x01/0x01 + any vendor-class (0xFF) interface — Android composite shape.
3. PHONE_VIDS allowlist (15 vendor IDs) paired with PTP or vendor-class.

Pure PTP-only DSLRs (Canon, Nikon) stay classified as Camera/Unknown.

## Test coverage added

- `pd::tests::reserved_bits_passive_detect` / `reserved_bits_active_only_bit_3`
- `cable::tests::trust_zero_vid_fires_on_blank_emarker`
- `cable::tests::trust_vid_unknown_fires_on_hex_fallback`
- `cable::tests::trust_reserved_bits_fires_on_dirty_vdo`
- `cable::tests::trust_clean_cable_fires_nothing`
- `summary::tests::classify_phone_android_via_adb_signature`
- `summary::tests::classify_phone_cat_s61_via_vid_and_ptp`
- `summary::tests::classify_camera_not_misidentified_as_phone`
- `summary::tests::transport_usb3_fires_for_superspeed`
- `summary::tests::transport_usb2_fires_for_highspeed`
- `summary::tests::transport_dp_and_tb_from_partner_altmodes`
- `tests/dbus_interface.rs` — `list_devices_carries_full_structured_fields`
  extended to assert `pdo_list` shape and values.

Tests grew from 102 to 150 (+13 new).

## Known follow-ups (out of scope for this break)

- **Sysfs reader doesn't infer `is_active` on PDOs.** The kernel publishes
  `current` (vs `maximum_current`) on the active PDO, but
  `src/sysfs/power.rs` always sets `is_active: false`. Until that's wired,
  `active_pdo_index` resolves to `-1` against real sysfs reads (in-memory
  fixtures and tests that set `is_active: true` explicitly work fine). This
  is a daemon-side enhancement, not a wire issue — UI gets a populated list,
  just no live active highlight yet.
- **USB4 detection.** `transport.tb` only fires on TBT3 altmode SVID 0x8087.
  USB4 itself doesn't appear as an altmode (it's PD-negotiated), so detecting
  it cleanly is a separate task — likely reading `port.usb_typec_rev` or
  `port.pd_revision`.
- **Cable plug altmodes.** Sibling altmode reader handles partner-side
  altmodes (`port0-partner.N`). Cable-plug altmodes (`port0-cable.N`,
  `port0-plug0.N`) aren't read yet — partner altmodes are the higher-value
  signal for the trust card.

## Verification

```
cargo build --all-features                          # clean
cargo test --all-features                           # 150 passed
cargo clippy --all-features --all-targets -- -D warnings   # clean
grep -rn "Devices2" src/ tests/ examples/ README.md AGENTS.md
  # Returns only the migration-guide prose hits in dbus.rs + README.md,
  # which are intentional ("Migrating from `Devices2`").
```
