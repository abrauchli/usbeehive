---
title: PD partner-symlink pairing + cable capability checks
status: complete
date: 2026-06-10
version_bump: none (stays 0.10.0 — unreleased)
interface_bump: none (stays org.usbeehive.Devices5 — additions only)
---

# Quick Task: PD partner pairing + no-e-marker hint + cable data-speed check

## Decisions (locked by user)

- Implement all three proposals from the capability audit.
- The Devices5 interface is **unreleased** — if anything here would break the
  wire, fold it into Devices5 / 0.10.0 rather than bumping to Devices6. (As
  planned, nothing below changes the signature: additions are `properties`
  keys, one `bottleneck` variant, and JSON-only fields, all non-breaking by
  the documented conventions in `src/dbus.rs`.)
- usbee (sister repo at `/home/blk/projects/rust/usbee`) is updated in a
  separate final task — label-table entries only, no wire change.

## Live-hardware evidence (verified 2026-06-10 on the dev laptop)

This machine (2 Type-C ports, 65W charger on port1) shows:

- `/sys/class/typec/port1-partner/usb_power_delivery` is a **symlink → pd2**.
  `pd2` holds the charger's full source-capabilities: 5V/3A, 9V/3A, 15V/3A,
  20V/3.25A (65W), PPS 5–21V/3A. The `pdN` nodes have **no `parent_port*`
  attribute** — the device-tree linkage (pd2's device path is
  `.../typec/port1/port1-partner/pd2`) and the partner symlink are the only
  pairing signals. usbeehive currently finds pd0/pd1/pd2 via
  `/sys/class/usb_power_delivery/` but pairs none of them with port1.
- BUG (confirmed by reading the code): `src/sysfs/power.rs::from_sysfs` never
  parses `parent_port_number`; `PowerDeliveryPort` **derives** `Default`, so
  every enumerated PD port gets `parent_port_number = 0`, which spuriously
  matches port **0** in `build_summaries` (`manager.rs:225`). The struct doc
  says "-1 if not linked". The test fixture (`tests/fixture_builder.rs:252`)
  writes a `parent_port_number` attr that the reader silently ignores —
  existing tests pass via the accidental 0-match or the single-port fallback.
- Live UCSI on port1: `voltage_now = 5_000_000`, `current_now = 3_000_000`
  (the laptop parks on the 5V/3A contract while battery-charge-limited; it
  does NOT hold 20V and lower the RDO current).
- `port1-cable` does not exist → no e-marker visible for the charger's cable.

## Scope (4 atomic tasks — one commit each)

### Task 1 — Pair PD caps via the partner `usb_power_delivery` symlink

**Files**: `src/typec.rs`, `src/sysfs/typec.rs`, `src/power.rs`,
`src/sysfs/power.rs`, `src/sysfs/manager.rs`, `tests/fixture_builder.rs`,
`tests/dbus_interface.rs` (assert only)

1. `src/typec.rs` — `TypeCPartner` gains `pd_name: String` (empty when no PD
   node is linked; plain String, not Option, to match the struct's style and
   keep `Default` derivable). Doc: basename of the partner's
   `usb_power_delivery` symlink target (e.g. `"pd2"`).
2. `src/sysfs/typec.rs` — when building the partner, resolve
   `partner_path.join("usb_power_delivery")` with `std::fs::read_link`; take
   the target's `file_name()` as `pd_name`. Fallback when the symlink is
   absent: scan partner subdirectories for a name matching `^pd[0-9]+$` and
   use the first (sorted) match. Both are best-effort — failures leave
   `pd_name` empty.
3. `src/power.rs` — replace the derived `Default` for `PowerDeliveryPort`
   with a manual impl setting `parent_port_number: -1` (everything else
   default). This kills the spurious port-0 match.
4. `src/sysfs/power.rs::from_sysfs` — parse the `parent_port_number` attr via
   `reader::read_int`, defaulting to `-1` when absent. (The attr is the
   fixture convention and harmless on real kernels that lack it.)
5. `src/sysfs/manager.rs::build_summaries` — pairing precedence becomes:
   1. partner symlink: `tc.partner.pd_name` non-empty and `== pd.name`;
   2. `pd.parent_port_number >= 0 && pd.parent_port_number == tc.port_number`;
   3. existing single-pd + single-port fallback.
   Keep the existing comment style; explain why symlink wins (kernel's
   canonical linkage; `parent_port_number` is fixture-only).
6. `tests/fixture_builder.rs` — `write_typec_partner` (or a new
   `link_partner_pd(root, port_name, pd_name)` helper) creates the
   `usb_power_delivery` symlink inside the partner dir via
   `std::os::unix::fs::symlink` (relative target is fine; only the basename
   is read).
7. Tests:
   - manager unit test: two ports + two pd nodes with `parent_port_number =
     -1`, partner on port1 carrying `pd_name = "pd1"` → port1 pairs with
     pd1, port0 pairs with nothing (regression for the spurious 0-match).
   - integration test in `tests/dbus_interface.rs`: fixture with TWO typec
     ports and one pd node with no `parent_port_number` attr, linked via the
     partner symlink → entry for that port carries the `pdo_list`,
     `charger_max`, and a non-`-1` `active_pdo_index` when a UCSI psy is
     present. This mirrors the live laptop exactly.

**Verification**: `cargo test --all-features`, then run
`cargo run --bin usbeehive` on the live machine — port1 must now show
"Charger profiles:" with five PDOs, the 5.0V profile marked active,
`Charger max: 65W`, `Charging in: up to 15W` and a diagnostic line.

### Task 2 — DeviceLimit hint + no-e-marker 3A heuristic

**Files**: `src/diagnostic.rs`, `src/summary.rs`, `src/output.rs`

1. `Bottleneck` gains `CableNoEMarker` (doc: contract current pinned at 3A
   while the selected PDO offers more and no e-marker is visible — cables
   without an e-marker are limited to 3A by spec). Adding a variant is
   non-breaking per the Devices5 conventions; update the variant list in the
   `DiagnosticEntry::bottleneck` doc in `src/dbus.rs`.
2. `ChargingDiagnostic::evaluate` gains a fourth param `requested_ma: u32`
   (RDO operating current in mA, 0 = unknown; crate-internal signature
   change, fine pre-release). New branch between `CableLimit` and
   `DeviceLimit`:
   - condition: no e-marker current rating (`cable.and_then(|c|
     c.current_rating).is_none()` — covers cable-absent too) AND active PDO
     `current_ma > 3_100` AND `requested_ma` in `2_900..=3_100`;
   - → `CableNoEMarker`, `is_warning: true`, summary
     "Cable may be limiting current to 3A", detail
     "Contract offers {pdo_ma/1000.}A but the device draws 3.0A and no cable
     e-marker is visible — cables without an e-marker are limited to 3A".
3. `DeviceLimit` detail gains the same benign hint SinkLimit has — append
   " — often the device's own policy (battery charge limit or thermal)".
   On the live laptop this is the diagnosis that fires (5V/3A contract of a
   65W advertisement) and it must not read as a cable accusation.
4. `src/summary.rs` — pass `current_now_ua / 1000` (clamped at 0) as
   `requested_ma`; additionally emit a soft property
   `("cable.no_emarker", "true")` when the port has a partner with PD source
   caps whose max advertised PDO current exceeds 3_000 mA AND no cable
   e-marker info exists. (Soft because some UCSI firmwares never populate
   cable nodes at all — phrase labels as "not visible", never "missing".)
5. `src/output.rs` — `property_flag_label` (or the existing flag-label
   table) gains `cable.no_emarker` → "No cable e-marker visible (3A limit
   may apply)", rendered DIM/normal, NOT yellow (it is a hint, not a trust
   warning).
6. Tests (diagnostic.rs): pinned-3A + 5A-PDO + no e-marker → CableNoEMarker
   with warning; same but WITH an e-marker rating → falls through (CableLimit
   or DeviceLimit per existing rules); requested 3.25A → no CableNoEMarker.
   Summary test: 65W charger + no cable → `cable.no_emarker` property
   present; 15W-only charger (max PDO 3A) → property absent.

### Task 3 — Cable data-speed cross-check (UFP VDO vs e-marker)

**Files**: `src/pd.rs`, `src/summary.rs`, `src/output.rs`

1. `src/pd.rs` — derive `PartialOrd, Ord` on `CableSpeed` (variants are
   already declared in ascending speed order; add a comment locking that
   invariant). New decoder:
   `pub fn decode_ufp_vdo_highest_speed(vdo1: u32) -> Option<CableSpeed>`:
   - Return `None` when `vdo1 == 0` or the UFP VDO version bits `[31:29]`
     are 0 (PD2.0 identities have no UFP VDO).
   - Bits `[2:0]` = USB Highest Speed: 0 → `Usb20`, 1 → `Usb32Gen1`,
     2 → `Usb32Gen2`, 3 → `Usb4Gen3`, 4 → `Usb4Gen4`, else `None`
     (reserved).
   - Only meaningful when the ID Header's `ufp_product_type` is
     `Hub`/`Peripheral` — gate at the call site.
2. `src/summary.rs` — for a connected Type-C port with BOTH:
   - partner identity where `ufp_product_type` ∈ {Hub, Peripheral} and
     `vdos.len() >= 4` → partner speed from
     `decode_ufp_vdo_highest_speed(vdos[3])`, and
   - cable info with `speed: Some(s)`,
   when `s < partner_speed`, emit property
   `("cable.data_speed_limit", cable_speed_label(s))` — e.g.
   `cable.data_speed_limit = "USB 2.0"`.
   Do NOT touch `ChargingDiagnostic` or `CapabilityDegraded` — those stay
   charging-only; this is a capability property the UI renders.
3. `src/output.rs` — value-bearing label for `cable.data_speed_limit`:
   "Cable limits data to {value}", colored YELLOW like the `cable.trust.*`
   warnings (a slow cable on a fast device is exactly the app's headline
   use case).
4. Tests: pd.rs decode tests (zero VDO, version-0 VDO, speed 0/2/4,
   reserved 7 → None; Ord sanity `Usb20 < Usb32Gen2 < Usb4Gen4`). Summary
   test: partner UFP VDO Gen2 + USB 2.0-only e-marked cable →
   `cable.data_speed_limit = "USB 2.0"`; same partner + Gen2 cable → absent;
   PD2.0 partner (vdo1 = 0) → absent.

### Task 4 — usbee label-table + CHANGELOG (sister repo, separate commit)

**Files** (in `/home/blk/projects/rust/usbee`):
`usbee@bitcreed.us/src/label-table.js`, `po/usbee@bitcreed.us.pot`,
`CHANGELOG.md`

1. label-table: add `cable.no_emarker` → "No cable e-marker" (flag-style
   key, renders as a plain row) and `cable.data_speed_limit` →
   "Cable data limit" (value-bearing). Unknown keys already render verbatim,
   so this is polish, not compatibility.
2. pot: add the two new label msgids (hand-edit like the "up to %s" entry —
   xgettext is not installed on this machine).
3. CHANGELOG: note under the unreleased 2.4.0 entry — daemon emits two new
   property keys; no wire change; MIN_USBEEHIVE_VERSION stays 0.10.0.

## Out of scope

- UCSI `voltage_max`/`current_max` as a contract fallback (live values are
  PPS-derived and unreliable; the partner symlink makes them unnecessary).
- Port↔USB-topology correlation for negotiated-link-speed checks (the UFP
  VDO comparison above covers the cable-vs-device capability case without
  it).
- Any wire signature change. CapabilityDegraded stays charging-only.

## CHANGELOG (usbeehive)

Fold everything into the existing unreleased `[0.10.0]` entry: new "Added"
bullets for partner-symlink pairing (with the parent-port default bugfix
called out), the `CableNoEMarker` bottleneck + `cable.no_emarker` property,
and `cable.data_speed_limit`. Do not create a new version heading.
