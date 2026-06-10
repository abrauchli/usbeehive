---
title: PD partner-symlink pairing + cable capability checks
status: complete
date: 2026-06-10
version_bump: none (folded into unreleased 0.10.0)
interface_bump: none (org.usbeehive.Devices5 unchanged — additive only)
---

# Summary

All four tasks executed and committed. Wire stayed Devices5 / crate 0.10.0
as decided — every change is additive (properties keys, one bottleneck
variant, JSON fields).

## Commits

usbeehive (master):
- `4c26eee` Task 1 — Pair PD capabilities with ports via the partner
  `usb_power_delivery` symlink. Includes the parent-port Default bugfix
  (spurious port-0 match) and parsing of the fixture `parent_port_number`
  attr. **Plus three real-kernel PD parsing fixes the fixtures had masked**:
  unit-suffixed PDO attrs (`5000mV`/`3000mA`), the runtime-PM `power/` dir
  inside `*-capabilities/` decoding as a junk PDO, and PPS index/type
  parsing from the directory name (`5:programmable_supply`).
- `1bf971d` Task 2 — `Bottleneck::CableNoEMarker` (3A-pinned heuristic,
  warning), `cable.no_emarker` soft property, DeviceLimit detail gains the
  benign battery-charge-limit hint, `evaluate()` takes `requested_ma`.
- `e45577f` Task 3 — `decode_ufp_vdo_highest_speed` (UFP VDO1 bits 2:0),
  `CableSpeed: Ord`, `cable.data_speed_limit` property when the e-marker
  speed is below the partner's advertised capability, YELLOW CLI label.

usbee (master):
- `ff9b4a6` Task 4 — label-table rows for both new keys ("Cable e-marker:
  not visible (3 A limit may apply)", "Cable data limit: <speed>"), pot
  entries, CHANGELOG. No wire/gate change; MIN_USBEEHIVE_VERSION stays
  0.10.0.

## Verification

- `cargo test --all-features`: 207 passed, 0 failed. clippy: 0 warnings.
- Live on the dev laptop (65W charger, port1): full five-PDO charger
  profile block now renders, 5.0V/3A marked active, `Charger max: 65W`,
  `Charging in: up to 15W`, DeviceLimit diagnostic with the benign hint,
  and the no-e-marker hint line. Before Task 1 this machine showed no
  charger capabilities at all.
- usbee: gjs dbus-client tests pass; label-table syntax-checked.

## Notes

- STATE.md does not exist in this repo's `.planning/` (precedent: the
  Devices2/Devices3 quick tasks) — no table update performed.
- The pre-existing uncommitted Devices5 work was committed first
  (usbeehive `61c542a`, usbee `c417b62`) so task commits stayed atomic.
