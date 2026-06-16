---
phase: quick-260616-box
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/sysfs/manager.rs
  - src/dbus.rs
  - src/bin/usbeehived.rs
  - CHANGELOG.md
autonomous: true
requirements: [QUICK-260616-box]
must_haves:
  truths:
    - "A status-only transition (Charging→Connected) on a still-present Type-C port populates SnapshotDiff.changed with that port's id"
    - "Raw wattage / voltage / current jitter alone (power_in_mw/contract_mw/pd_contract) does NOT populate changed"
    - "A role / transport / link-speed change on a present device populates changed"
    - "The daemon emits a DeviceChanged(id) D-Bus signal for each id in diff.changed, with a matching stderr trace line"
    - "The interface name stays org.usbeehive.Devices5 — no wire/version bump (additive signal only)"
  artifacts:
    - path: "src/sysfs/manager.rs"
      provides: "SnapshotDiff.changed field + curated state fingerprint in Snapshot::diff + unit tests"
      contains: "changed"
    - path: "src/dbus.rs"
      provides: "DeviceChanged(id: String) zbus signal on DevicesIface"
      contains: "device_changed"
    - path: "src/bin/usbeehived.rs"
      provides: "emit_signals block emitting DeviceChanged for diff.changed"
      contains: "device_changed"
    - path: "CHANGELOG.md"
      provides: "Entry documenting the additive DeviceChanged signal (no wire bump)"
      contains: "DeviceChanged"
  key_links:
    - from: "src/sysfs/manager.rs Snapshot::diff"
      to: "SnapshotDiff.changed"
      via: "state_fingerprint comparison over ids present in both snapshots"
      pattern: "diff.changed|state_fingerprint"
    - from: "src/bin/usbeehived.rs emit_signals"
      to: "DevicesIface::device_changed"
      via: "block_on emit loop over diff.changed"
      pattern: "device_changed"
---

<objective>
Fix the stale "Charging 15W" tile in the USBee GNOME indicator after the
laptop's USB-C power source is unplugged. The daemon (usbeehive) currently
emits no D-Bus signal when a *present* device/port changes user-visible
state (e.g. Charging → Connected), so pure-snapshot consumers never know to
re-fetch. This plan adds an additive, curated `DeviceChanged(id)` signal on
the existing `org.usbeehive.Devices5` interface.

Purpose: give D-Bus consumers a wake-up on benign present-port state
transitions without sub-1s wakeups from raw mW/mV polling jitter.

Output: a new `SnapshotDiff.changed` list computed from a curated state
fingerprint, the `DeviceChanged` zbus signal, daemon emission wiring,
unit tests, and a CHANGELOG entry.

Out of scope (tracked separately, in the ../usbee repo): the matching USBee
consumer change to subscribe to `DeviceChanged` → call its re-snapshot
(`_scheduleRefresh`) with NO desktop notification. Do NOT touch ../usbee.
</objective>

<execution_context>
@$HOME/.claude/gsd-core/workflows/execute-plan.md
@$HOME/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@CLAUDE.md
@src/sysfs/manager.rs
@src/summary.rs
@src/dbus.rs
@src/bin/usbeehived.rs
@CHANGELOG.md

Verified field locations (from reading the structs):
- `status` → `DeviceSummary.status` (`Status` enum: Empty | Connected | Charging | Sourcing)
- `power_role` → `DeviceSummary.power.power_role` (`PowerRole` enum)
- `data_role` → `DeviceSummary.properties` entry keyed `"data_role"`
- transport flags → `DeviceSummary.properties` keys prefixed `"transport."`
  (`transport.usb2`, `transport.usb3`, `transport.usb4`, `transport.dp_altmode`, `transport.tb`)
- `link_speed_mbps` → `DeviceSummary.link_speed_mbps` (u32)
- `usb_version` → `DeviceSummary.usb_version` (String)
- active PDO index → position of `is_active` in
  `DeviceSummary.power_delivery.source_capabilities` (or -1)
- `primary_driver` → `DeviceSummary.primary_driver` (String)
- EXCLUDE from fingerprint (jitter): `power.power_in_mw`, `power.power_out_mw`,
  `power.contract_mw`, and the `"pd_contract"` property (carries live
  `5.0V @ 3.00A`), plus `"charger_max"` / any raw-magnitude property.
- `DeviceSummary::id()` yields `typec:<port_name>` / `usb:<bus_port>`.
- `SnapshotDiff` is re-exported at `usbeehive::SnapshotDiff` (src/lib.rs).
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Add SnapshotDiff.changed + curated state fingerprint in Snapshot::diff</name>
  <files>src/sysfs/manager.rs</files>
  <behavior>
    - A Type-C port present in BOTH snapshots that flips Status::Charging →
      Status::Connected populates `changed` with its id, and leaves
      `added`/`removed`/`newly_degraded`/`resolved` empty.
    - Two snapshots of the same present port that differ ONLY in raw power
      magnitudes (power_in_mw / contract_mw) and the `pd_contract` property
      produce empty `changed` (no jitter wakeup).
    - A present device whose `link_speed_mbps` (or data_role / a transport.*
      flag / primary_driver / active PDO index) changed populates `changed`.
    - An added or removed id is NOT also reported in `changed` (changed is
      computed only for ids present in both snapshots).
    - `is_empty()` returns false when only `changed` is non-empty.
  </behavior>
  <action>
    In src/sysfs/manager.rs: add a `pub changed: Vec<String>` field to
    `SnapshotDiff` (after `resolved`) and update its doc comment to describe
    it as "Summary ids present in BOTH snapshots whose curated user-visible
    state changed (status, power/data role, transport flags, link speed, USB
    version, active PDO index, primary driver) — excludes raw power
    magnitudes so 500ms mW/mV jitter does not wake consumers." Update
    `SnapshotDiff::is_empty()` to also require `self.changed.is_empty()`.

    Add a private free function `fn state_fingerprint(s: &DeviceSummary) ->
    String` (place it near `Snapshot::diff`, module-private). Build a
    deterministic string from ONLY the curated fields, formatted so distinct
    states never collide and identical states always match:
      - `format!("{:?}", s.status)`
      - `format!("{:?}", s.power.power_role)`
      - `s.link_speed_mbps` and `s.usb_version`
      - `s.primary_driver`
      - active PDO index: `s.power_delivery.as_ref().and_then(|pd|
        pd.source_capabilities.iter().position(|p| p.is_active)).map(|i| i
        as i32).unwrap_or(-1)`
      - a curated, ORDER-STABLE subset of `s.properties`: include only the
        `"data_role"` key and any key starting with `"transport."`. Collect
        the matching `(k, v)` pairs, SORT them (sort_by key) so source-order
        churn never flips the fingerprint, and fold into the string.
    Do NOT include any raw-magnitude property (`pd_contract`, `charger_max`,
    `usb_max_power_ma`, etc.) or any `power.*_mw` field.

    In `Snapshot::diff`: after computing `added`/`removed` and the
    degraded/resolved pass, compute `changed`. Build a HashMap<String,
    String> of `previous` summaries' `id() -> state_fingerprint(s)`. Iterate
    `self.summaries`; for each `s` whose id is in `prev_ids` AND not in
    `added` (i.e. present in both), compare `state_fingerprint(s)` against
    the previous fingerprint for that id; push `s.id()` to `changed` when
    they differ. Add `changed` to the returned `SnapshotDiff { .. }`.

    Then add unit tests in the existing `#[cfg(test)] mod tests` following
    the `snapshot_diff_*` style already in the file:
      (a) `snapshot_diff_flags_status_only_transition_as_changed` — build a
          present Type-C port summary in `prev` with `status =
          Status::Charging`, and the same id in `cur` with `status =
          Status::Connected` (mutate the built summary's `.status` directly,
          as the degraded test mutates `.charging_diag`). Assert
          `diff.changed == vec!["typec:<name>"]` (use whatever id the port
          produces) and that added/removed/newly_degraded/resolved are empty
          and `!diff.is_empty()`.
      (b) `snapshot_diff_ignores_raw_power_jitter` — same id in both, equal
          curated state, but mutate only `cur` summary's `power.power_in_mw`
          / `power.contract_mw` and push a differing `("pd_contract", ..)`
          property. Assert `diff.changed.is_empty()`.
      (c) `snapshot_diff_flags_link_speed_change_as_changed` — same id, vary
          `link_speed_mbps` (e.g. 5000 → 10000) between prev and cur. Assert
          the id is in `diff.changed`.
    Use `DeviceSummary::id()` to derive expected ids rather than hardcoding
    where the port_name is empty in fixtures; prefer a port whose id is
    stable, mirroring existing tests.

    NOTE: do NOT place fenced code in this file's prose — write the Rust
    directly into manager.rs.
  </action>
  <verify>
    <automated>cargo test --lib sysfs::manager 2>&1 | tail -30; cargo test --lib snapshot_diff 2>&1 | tail -30</automated>
  </verify>
  <done>SnapshotDiff has a `changed: Vec<String>` field; `is_empty()` accounts for it; `Snapshot::diff` populates `changed` via the curated fingerprint for ids present in both snapshots; the three new tests pass and all pre-existing `snapshot_diff_*` tests still pass.</done>
</task>

<task type="auto">
  <name>Task 2: Declare DeviceChanged signal and emit it from the daemon</name>
  <files>src/dbus.rs, src/bin/usbeehived.rs</files>
  <action>
    In src/dbus.rs: declare a new zbus signal on the `#[interface(name =
    "org.usbeehive.Devices5")] impl DevicesIface` block, mirroring
    `device_removed` (payload is a single `id: String`). Add it AFTER
    `device_removed` / before `capability_degraded`:
      a `#[zbus(signal)] pub async fn device_changed(emitter:
      &SignalEmitter<'_>, id: String) -> zbus::Result<()>;` with a doc
      comment: "Emitted when a device/port that remains present changes
      user-visible state (status, role, transport, link speed, active PDO,
      driver). Additive — existing consumers ignore it; new consumers
      re-snapshot. Excludes raw power-magnitude jitter."
    Also update the module-level docs: add a row to the "Methods,
    properties, signals" table — `| `DeviceChanged` (signal) | `s` | `id`
    of a present device/port whose curated user-visible state changed.
    Additive (0.10.x); does not bump the interface. |`. Do NOT change the
    interface name, the `ListDevices` signature, or any existing member.

    In src/bin/usbeehived.rs `emit_signals`: add an emission loop for
    `diff.changed`, mirroring the `diff.removed` block (same single-id
    shape). Place it after the `diff.removed` loop and before the
    `diff.newly_degraded` loop. For each `id` in `&diff.changed`: print a
    stderr trace line (e.g. `eprintln!("usbeehived: ~ {id} changed");`,
    using a distinct marker from `+`/`-`/`!`/`✓`) and
    `block_on(DevicesIface::device_changed(emitter, id.clone()))`, logging
    an emit-failure line on `Err` like the other blocks. Also update the
    module doc comment listing emitted signals to include `DeviceChanged`.
    No interface bump, no MIN_USBEEHIVE_VERSION change.
  </action>
  <verify>
    <automated>cargo build --no-default-features --features dbus 2>&1 | tail -20 && cargo test --no-default-features --features dbus 2>&1 | tail -20</automated>
  </verify>
  <done>`DevicesIface` exposes a `device_changed(id)` zbus signal under the unchanged `org.usbeehive.Devices5` interface; `emit_signals` emits it for every id in `diff.changed` with a stderr trace; the `dbus`-featured build and tests compile and pass.</done>
</task>

<task type="auto">
  <name>Task 3: Document the additive DeviceChanged signal in CHANGELOG</name>
  <files>CHANGELOG.md</files>
  <action>
    In CHANGELOG.md: add a new section ABOVE the existing `## [0.10.0]`
    entry. Since this is an additive, non-breaking interface addition (no
    wire bump, interface stays `org.usbeehive.Devices5`), use an
    `## [Unreleased]` section (the file currently has none) with an
    `### Added` subsection. Document: a new `DeviceChanged(id: String)`
    D-Bus signal on `org.usbeehive.Devices5`, emitted when a device/port
    that remains present changes user-visible state (status, power/data
    role, transport flags, link speed, USB version, active PDO index,
    primary driver). State explicitly: it is computed from a CURATED state
    fingerprint that EXCLUDES raw power magnitudes (mW/mV) so the daemon's
    ~500ms poll jitter does not wake consumers; it is additive and does NOT
    bump the interface version or `MIN_USBEEHIVE_VERSION`; existing
    consumers ignore the unknown signal. Mention the motivation: lets
    pure-snapshot consumers (the USBee GNOME indicator) re-snapshot after a
    benign present-port transition such as AC unplug, fixing a stale
    "Charging NW" tile. Match the existing CHANGELOG prose style and add an
    `[Unreleased]` reference link at the bottom if the file uses reference
    links. Do NOT bump the Cargo `version` field — that happens at release
    time per the project Release Process.
  </action>
  <verify>
    <automated>grep -q "DeviceChanged" CHANGELOG.md && grep -qi "Unreleased" CHANGELOG.md && echo OK</automated>
  </verify>
  <done>CHANGELOG.md has an `## [Unreleased]` → `### Added` entry documenting the additive `DeviceChanged` signal, the curated-fingerprint / no-jitter behavior, and the explicit "no interface/version bump" note.</done>
</task>

</tasks>

<verification>
- `cargo test` (default features) passes, including the three new
  `snapshot_diff_*` tests.
- `cargo build --no-default-features --features dbus` and
  `cargo test --no-default-features --features dbus` both pass.
- `grep "device_changed" src/dbus.rs src/bin/usbeehived.rs` shows the
  signal declaration and its emission.
- `grep -n "Devices5" src/dbus.rs` confirms the interface name is unchanged
  (no `Devices6`).
</verification>

<success_criteria>
- `SnapshotDiff.changed: Vec<String>` exists, is folded into `is_empty()`,
  and is populated only for ids present in both snapshots whose curated
  fingerprint changed.
- Raw mW/mV/pd_contract jitter alone never populates `changed`.
- The daemon emits `DeviceChanged(id)` for each changed id, with a stderr
  trace, over the unchanged `org.usbeehive.Devices5` interface.
- CHANGELOG documents the additive signal with the no-bump guarantee.
- The matching ../usbee consumer change remains untouched (out of scope).
</success_criteria>

<output>
Create `.planning/quick/260616-box-emit-devicechanged-signal-so-consumers-r/260616-box-SUMMARY.md` when done.
</output>
