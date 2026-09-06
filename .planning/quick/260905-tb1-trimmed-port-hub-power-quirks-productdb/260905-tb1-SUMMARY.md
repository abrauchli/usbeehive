---
quick_id: 260905-tb1
slug: trimmed-port-hub-power-quirks-productdb
date: 2026-09-05
status: complete
commits:
  - 37310e1 feat(usb) read physical connector, power source and hub occupancy
  - eeec101 feat surface connector / power-budget / kernel keys (additive, Devices5)
  - cb82ade docs consumer spec + CHANGELOG for the connector / power / quirk keys
---

# Summary — 260905-tb1

## What shipped

Exactly the 11 keys the user selected. Nothing outside the trim.

| Key | Live example (this machine) |
|---|---|
| `port.id` | `usb5-port2` (`5-2`), `5-2.4-port1` (`5-2.4.1`) |
| `port.peer_id` | `usb6-port2` (`5-2`) |
| `port.peer_state` | `not attached` (`5-2`) |
| `port.connect_type` | `hotplug` (`5-2`), `hardwired` (`3-6`) |
| `hub.ports_total` | `4` (`5-2`), `3` (`5-2.4`) |
| `hub.ports_used` | `2` (`5-2`), `0` (`3-6`) |
| `power.source` | `self` (`5-2`), `bus` (`5-2.4`) |
| `hub.power_budget_ma` | `500` (`5-2.4`) |
| `hub.power_committed_ma` | `188` (`5-2.4`), `100` (`5-2`) |
| `kernel.quirks` | `NO_LPM` (`5-2.1.1`) |
| `product_db` | `RTS5411 Hub` (`5-2`), `AX200 Bluetooth` (`1-4`), `RTL8153 Gigabit Ethernet Adapter` (`5-2.1.1`) |

**Model** (`src/usb.rs`): eight additive serde-default `UsbDevice` fields —
`port_id`, `port_peer_id`, `port_peer_state`, `port_connect_type`,
`max_child`, `ports_used: Option<u32>`, `bm_attributes`, `product_db` — plus
`quirk_names()` (Linux 7.0 bits 0–18, unknown bits render `bit<N>`),
`self_powered()`, `hub_power_budget_ma()`, `hub_power_committed_ma()`.

**Reads** (`src/sysfs/usb.rs`): `port` symlink → `connect_type` → `peer`
symlink → peer `state`; hub occupancy from the port objects under the hub's
interface directory; `bmAttributes`; `product_db` via
`udev::Device::from_syspath` + `ID_MODEL_FROM_DATABASE`, behind
`cfg(feature = "watch")` with a no-op fallback.

**Wire** (`src/summary.rs`, `src/sysfs/manager.rs`, `src/output.rs`,
`src/dbus.rs`): the 11 keys, the fingerprint change, CLI display labels, and
the `dbus.rs` wire-surface documentation table.

## Interface

`org.usbeehive.Devices5`, unchanged. No tuple shape, method signature, enum
value, or field position was touched. No new signal. `MIN_USBEEHIVE_VERSION`
does not need to move.

## Fingerprint decisions

`state_fingerprint()` now keys off a named `FINGERPRINTED_PROPERTY_KEYS`
list instead of an inline predicate.

- **In:** `port.peer_state` (normative SHOULD in the spec — a companion port
  training late is the transition that flips a BOS verdict), plus the static
  `power.source` and `kernel.quirks`.
- **Out:** `hub.ports_used` and `hub.power_committed_ma`. They move only on
  plug/unplug, which already fires `DeviceAdded` / `DeviceRemoved`;
  fingerprinting them would add a redundant `DeviceChanged` on the hub for
  the same event. Also out: the remaining static keys, whose change implies
  re-enumeration under a new id anyway.

Three tests pin this: `connector_keys_do_not_make_devicechanged_chatter_when_idle`,
`peer_port_training_late_fires_devicechanged`, and
`hub_occupancy_change_alone_does_not_fire_devicechanged_on_the_hub`.

Live evidence: 60 consecutive `DeviceManager::refresh()` passes over 15 s
against real `/sys` with 11 summaries produced **zero** diffs of any kind.

## Two presence refinements (documented, not silent)

1. `port.connect_type` omits the literal `unknown` the kernel writes on every
   hub-internal port — absence already means unknown in this key vocabulary,
   and emitting it would put a useless value on 6 of 11 live entries.
2. `product_db` is omitted when the hwdb name is contained
   (case-insensitively) in the device's own `iProduct`: hwdb answers `Hub`
   next to `USB2.0 Hub` (noise) but `RTS5411 Hub` next to
   `4-Port USB 2.0 Hub` (names the silicon).

Both are recorded in `DBUS-TRIM-CONSUMER-SPEC.md` §7.2 / §7.4 and in the
status header of `DBUS-FURTHER-SPEC.md`.

## Deferred, as instructed

All of §C (`pm.*`, `connected_s`), §E (`function.*`), §F
(`interface.<N>.name`, `usb.configurations`), the `PortOverCurrent` signal
(§H), and `hub.multi_tt`, `power.remote_wakeup_capable`,
`kernel.avoid_reset`, `authorized`, `port.disabled`. A D-Bus test asserts
none of them appears in the bag, so the trim cannot drift.

Nothing deferred turned out to be required for correctness of what shipped.
`bmAttributes` bit 5 (remote wakeup) is readable from
`SnapshotJson.usb_device.bm_attributes` for a client that wants it before
the key is promoted.

## Verification

`rtk proxy` used throughout (the token filter strips `warning:` and test
result lines, so a naive grep passes vacuously).

- `cargo fmt --all --check` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo test --all-features` — 224 + 12 + 5 + 7 + 15 + 0 + 5 + 18 + 12 = 298 passed, 0 failed.
- `cargo test` (default) — 218 + 12 + 7 + 0 + 0 + 5 + 18 + 12 = 272 passed, 0 failed.
- `cargo test --no-default-features --features dbus` — 224 + 5 + 0 + 15 + 0 + 5 + 18 + 12 = 279 passed, 0 failed.
- `cargo test --no-default-features` — 168 + 7 = 175 passed, 0 failed.
- `cargo test --no-default-features --features sysfs` (extra: exercises the
  no-udev `product_db` fallback) — 216 + 5 + 18 + 10 = 249 passed, 0 failed.
- Live: `usbeehive --list` against real `/sys` renders every key on the
  expected devices.

No version bump, no tag, no push. `../usbee` untouched.
