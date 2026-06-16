---
phase: quick-260616-box
plan: 01
subsystem: dbus-signals
tags: [dbus, snapshot-diff, signals, tdd]
dependency_graph:
  requires: []
  provides: [DeviceChanged-signal, SnapshotDiff.changed, state_fingerprint]
  affects: [src/sysfs/manager.rs, src/dbus.rs, src/bin/usbeehived.rs, CHANGELOG.md]
tech_stack:
  added: []
  patterns: [curated-state-fingerprint, tdd-red-green]
key_files:
  created: []
  modified:
    - src/sysfs/manager.rs
    - src/dbus.rs
    - src/bin/usbeehived.rs
    - CHANGELOG.md
decisions:
  - "Exclude raw power magnitudes from fingerprint to suppress ~500ms poll jitter"
  - "Sort curated properties by key before folding into fingerprint string"
  - "Place DeviceChanged emission loop between removed and newly_degraded loops"
  - "Interface stays org.usbeehive.Devices5 — additive signal requires no wire bump"
metrics:
  duration: "~20 minutes"
  completed: "2026-06-16"
  tasks_completed: 3
  files_modified: 4
---

# Phase quick-260616-box Plan 01: Emit DeviceChanged Signal Summary

**One-liner:** Additive DeviceChanged(id) D-Bus signal on org.usbeehive.Devices5 driven by a curated state fingerprint that excludes raw mW/mV jitter, fixing stale "Charging NW" tiles in pure-snapshot consumers.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (RED) | Add failing tests for SnapshotDiff.changed | 0a9070c | src/sysfs/manager.rs |
| 1 (GREEN) | Add SnapshotDiff.changed + curated state fingerprint | 0874f36 | src/sysfs/manager.rs |
| 2 | Declare DeviceChanged signal and emit from daemon | b58fc26 | src/dbus.rs, src/bin/usbeehived.rs |
| 3 | Document additive DeviceChanged signal in CHANGELOG | 3b56397 | CHANGELOG.md |

## What Was Built

### Task 1: SnapshotDiff.changed + state_fingerprint (TDD)

RED: Added three failing tests to src/sysfs/manager.rs:
- snapshot_diff_flags_status_only_transition_as_changed: Charging->Connected populates changed
- snapshot_diff_ignores_raw_power_jitter: power_in_mw/contract_mw/pd_contract mutation alone does not
- snapshot_diff_flags_link_speed_change_as_changed: link_speed_mbps change populates changed

GREEN: Implemented in src/sysfs/manager.rs:
- pub changed: Vec<String> added to SnapshotDiff (after resolved)
- SnapshotDiff::is_empty() updated to require self.changed.is_empty()
- Private fn state_fingerprint(s: &DeviceSummary) -> String: covers status, power_role,
  link_speed_mbps, usb_version, primary_driver, active PDO index, and sorted curated
  properties (data_role + transport.* keys only)
- Snapshot::diff builds HashMap of prev fingerprints; populates changed for ids present
  in both snapshots whose fingerprint differs

All 18 manager tests pass.

### Task 2: DeviceChanged signal declaration and emission

src/dbus.rs:
- Added device_changed(emitter, id: String) zbus signal after device_removed
- Updated Methods/properties/signals table in module docs
- Interface org.usbeehive.Devices5 unchanged

src/bin/usbeehived.rs:
- Added emission loop for diff.changed between removed and newly_degraded blocks
- Trace: eprintln!("usbeehived: ~ {id} changed") with distinct ~ marker
- Emit failure logged: "usbeehived: device_changed emit failed: {e}"
- Module doc updated to list DeviceChanged

### Task 3: CHANGELOG documentation

Added ## [Unreleased] section above ## [0.10.0] with ### Added subsection documenting:
- DeviceChanged(id: String) signal and its curated-fingerprint scope
- Excluded fields (raw power magnitudes + mV/mW properties)
- Additive status: no interface bump, no MIN_USBEEHIVE_VERSION change
- Motivation: fixes stale "Charging NW" tile in usbee GNOME indicator
Added [Unreleased] and [0.10.0] reference links at the bottom.

## Verification Results

- cargo test: ok. 18 lib + 10 doc-tests passed
- cargo build --no-default-features --features dbus: Finished (no errors)
- cargo test --no-default-features --features dbus: ok. 10 doc-tests passed
- grep "device_changed" src/dbus.rs src/bin/usbeehived.rs: signal + emission found
- grep -n "Devices5" src/dbus.rs: interface name unchanged, no Devices6
- grep "DeviceChanged" CHANGELOG.md && grep -qi "Unreleased" CHANGELOG.md: OK

## Decisions Made

1. Fingerprint excludes raw mW/mV: power_in_mw, power_out_mw, contract_mw, pd_contract,
   charger_max are excluded. These jitter on every poll cycle and do not represent
   user-visible state transitions.

2. Properties sorted before folding: data_role + transport.* entries sorted by key so
   fingerprint is deterministic regardless of insertion order in build_summaries.

3. DeviceChanged positioned between removed and newly_degraded: logical ordering of
   appearances -> disappearances -> transitions -> degradations -> restorations.

4. No interface bump: signal is additive. Clients that do not subscribe receive nothing.

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None - no new network endpoints, auth paths, file access patterns, or schema changes.
The new signal is an additive output-only wire event on the existing session-bus interface.

## Self-Check: PASSED

- src/sysfs/manager.rs modified and committed at 0874f36 (GREEN), 0a9070c (RED test)
- src/dbus.rs modified and committed at b58fc26
- src/bin/usbeehived.rs modified and committed at b58fc26
- CHANGELOG.md modified and committed at 3b56397
- All 4 task commits confirmed: 0a9070c, 0874f36, b58fc26, 3b56397
