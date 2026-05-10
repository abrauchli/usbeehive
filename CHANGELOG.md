# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-05-09

**Project renamed: `whatcable` → `usbeehive`.** At the request of the
original WhatCable author Darryl Morley, this Rust port no longer ships
under the `whatcable` name. The project, crate, CLI binary, daemon, and
D-Bus interface have all been renamed; thanks to Darryl for suggesting
the new name.

### Changed

- **Crate name:** `whatcable` → `usbeehive` (install with
  `cargo install usbeehive`). The previous `whatcable` crate is retired
  on crates.io with a redirect README; older versions are yanked.
- **CLI binary:** `whatcable` → `usbeehive`.
- **Daemon binary:** `whatcabled` → `usbeehived`.
- **D-Bus bus name / object path / interface:**
  `org.whatcable.Devices` → `org.usbeehive.Devices`,
  `/org/whatcable/Devices` → `/org/usbeehive/Devices`,
  `org.whatcable.Devices1` → `org.usbeehive.Devices1`. Existing D-Bus
  clients must update their proxy bindings.
- **Repository home:** [`abrauchli/whatcable`](https://github.com/abrauchli/whatcable)
  → [`abrauchli/usbeehive`](https://github.com/abrauchli/usbeehive). Full
  history was preserved in the move; the old repo is archived and points
  here.

### Added (carried over from unreleased)

- **Optional D-Bus interface (`dbus` feature).** New `usbeehive::dbus`
  module exposing `org.usbeehive.Devices1` (object path
  `/org/usbeehive/Devices`) with `ListDevices`, `ListPorts`, `Diagnose`,
  `SnapshotJson`, `Refresh` methods, `Version` / `DeviceCount`
  properties, and `DeviceAdded` / `DeviceRemoved` /
  `CapabilityDegraded` / `CapabilityRestored` signals. Disabled by
  default — pulls in `zbus` only when requested.
- **`usbeehived` daemon binary** (built with `--features dbus`). Runs
  the libudev hot-plug loop in a background thread and emits D-Bus
  signals as the snapshot changes. Suppresses signals on the initial
  baseline refresh so already-plugged devices don't re-fire as fresh
  events.
- **`Snapshot::diff(&Snapshot) -> SnapshotDiff`** — added/removed
  device ids plus newly-degraded / resolved Type-C port numbers. Backs
  the daemon's signal classification but is independently useful to
  library consumers.
- **`DeviceSummary::id() -> String`** — stable
  `"typec:<port_name>"` / `"usb:<bus_port>"` identifier, used as the
  diff key.
- `examples/dbus_client.rs` — minimal `zbus` client that lists devices
  and queries port 0's diagnostic.

### Migration from `whatcable` 0.4.0

```toml
# Cargo.toml — change the dependency name and version:
- whatcable = "0.4"
+ usbeehive = "0.5"
```

```rust
// Code — global rename:
- use whatcable::{DeviceManager, pd::*};
+ use usbeehive::{DeviceManager, pd::*};
```

```sh
# D-Bus clients — update bus name and interface:
- gdbus call --session --dest org.whatcable.Devices …
+ gdbus call --session --dest org.usbeehive.Devices …
```

The library API is otherwise identical to `whatcable` 0.4.0 plus the
D-Bus additions listed above; no breaking type / function changes
beyond the rename.

## [0.4.0] - 2026-05-08

Topology + link-speed surface: the USB tree view is now the default CLI
output, and the helpers that drive it are public on the library.

### Added

- **Tree topology view as the new default CLI output.** Renders the USB
  bus with `├─ └─` branches; each device is colored by its upstream link
  speed (gray → magenta, Low Speed through USB4) so bottlenecks are
  visible at a glance without per-line speed labels. Includes a legend
  at the bottom and italic styling for hubs. Empty root hubs are hidden;
  orphan devices (no enumerated parent, e.g. fixture-only) are still
  rendered as top-level roots.
- `--list` flag — restores the previous flat per-device view.
- `--tree` flag — explicit form of the new default.
- **`UsbDevice::parent_bus_port() -> Option<String>`** — sysfs
  identifier of the parent device, `None` for kernel root hubs.
- **`usb::tree_roots(&[UsbDevice]) -> Vec<&UsbDevice>`** — devices with
  no parent in the slice (root hubs + orphans), the entry points for a
  topology walk over `UsbDevice::children`.
- **`LinkSpeed` enum + `link_speed_tier(u32) -> LinkSpeed` +
  `UsbDevice::link_speed_tier()`** — a stable, switchable tier instead
  of raw Mbps thresholds. Adds a 40 Gbps USB4 tier the previous
  bucketing collapsed into "USB4 20 Gbps".

### Changed

- `speed_label(u32)` is now `link_speed_tier(speed).label()`. Existing
  labels (and their tests) are preserved verbatim.
- `src/sysfs/usb.rs::build_topology` and `src/output.rs` (tree renderer)
  drop their private parent-resolution / root-collection helpers in
  favor of the public API.

## [0.3.1] - 2026-05-02

Metadata-only patch release.

### Changed

- `repository` and `homepage` in `Cargo.toml` now point to
  `github.com/abrauchli/whatcable` (the active fork at the time;
  later renamed to `abrauchli/usbeehive`) instead of the upstream
  Zetaphor repo.
- MSRV correctly declared as `1.85` (was `1.74` in 0.3.0; the actual
  build hasn't worked on 1.74 since `clap_lex 1.1` adopted edition
  2024). Users running stable Rust are unaffected.

## [0.3.0] - 2026-05-02

Major rewrite: the crate gains a real library API behind feature flags,
a fixture-driven test suite, and several long-standing Type-C parsing
bugs are fixed. Same install path (`cargo install whatcable`), same
binary name (`whatcable`).

### Added

- **Feature-gated library API.** Three layers, each toggled by a Cargo feature:
  - **(default-off `cli`)** — the `whatcable` binary (clap + JSON / text rendering).
  - **(default-on `sysfs`)** — Linux `/sys` enumeration: `Sysfs` handle with
    injectable root, `DeviceManager`, `Snapshot`, `Error`, `Result`.
  - **(default-on `watch`)** — libudev hotplug: `watch::Watcher`,
    `watch::run_loop`, debounced render loop with `SIGINT` / `SIGTERM` handling.
  - Pure-decoder library use: `default-features = false`. Only `serde` is pulled.
- **`Sysfs::with_root(path)`** — injectable sysfs root for fixture-based testing
  and offline analysis. `Sysfs::try_with_root(path)` validates the path is a
  directory, returning `Error::InvalidRoot` otherwise.
- **`DeviceManager::with_sysfs(sysfs)`** + `Snapshot` struct — exposes the
  structured `usb_devices`, `typec_ports`, `pd_ports`, and `summaries`
  together so callers don't have to reach through accessor methods.
- **`TypeCPowerSupply::negotiated_power_mw()`** — `i128`-safe live wattage
  helper, computed from `voltage_now × current_now`. Used by both the
  human-readable bullet and the JSON `negotiatedPowerMW` field.
- **`--sysfs-root <PATH>`** CLI flag, useful for running the binary against
  captured fixture trees (and used by the new end-to-end smoke tests).
- Five runnable `examples/` — `decode_cable_vdo`, `cable_info`,
  `list_devices`, `snapshot_diff`, `print_changes`.
- 98 tests, up from ~30. Includes end-to-end CLI smoke tests that build a
  sysfs tree on disk and exercise the binary, plus integration tests
  covering charging diagnostics, PPS PDO parsing, deep USB topology, and
  cable-bottleneck detection.
- New CI workflow (`.github/workflows/ci.yml`): test matrix
  (default + `--no-default-features`), `cargo fmt --check`, `cargo clippy
  --all-targets -D warnings`, MSRV (1.85) build, rustdoc with `-D warnings`.
- Crate-level doc comments on every public item; runnable doctest in `lib.rs`.
- This `CHANGELOG.md`.

### Changed

- Library reorganisation. The previous flat `whatcable::*` re-exports remain
  the recommended entry point (`whatcable::DeviceManager`, `whatcable::pd::…`),
  but internal modules moved: sysfs IO is now under `whatcable::sysfs::*`.
- `ProductType` and `PdoType` use `#[derive(Default)]` + `#[default]` rather
  than hand-written `impl Default`.
- Workspace-wide rustfmt; clippy clean under `-D warnings`.

### Fixed

These four bugs existed in 0.2.1 but were only surfaced by the new
fixture-driven integration tests:

- **Type-C `read_identity` pushed VDOs in alphabetical filename order**
  (`cert_stat`, `id_header`, `product`, …) instead of USB-PD spec order
  (`id_header` first). Decoders treat `vdos[0]` as the ID Header and
  `vdos[3]` as the Cable VDO, so this silently mis-decoded every real
  cable / partner read from sysfs. Now walks a fixed `SPEC_ORDER` list,
  padding missing slots with `0` for stable indexing.
- **Type-C port enumeration accepted sibling entries** like
  `port0-partner` / `port0-cable` / `port0-plug0` as if they were ports,
  because the filter only checked `name.starts_with("port")` without
  requiring the trail to be numeric. Fix: reject any sibling whose
  trailing characters aren't all ASCII digits.
- **PPS PDO parsing used `voltage_mv = 0`** unless `maximum_voltage` was
  also missing. PPS / variable-supply PDOs publish `minimum_voltage` +
  `maximum_voltage` instead of a single `voltage` file; the parser now
  falls back to `minimum_voltage` whenever `voltage` itself is absent.
- **Deep USB topology dropped intermediate nodes**: `build_topology`
  cloned children from a pre-snapshot, so a `5-2.1.1` device ended up in
  the flat list, but its parent `5-2.1` was attached to `5-2.children`
  with empty `.children`. Fix: process devices deepest-first.

### Migration from 0.2.1

For most users — `cargo install whatcable` works exactly as before;
binary behavior is unchanged.

For library consumers:

- The previous `UsbDevice::enumerate()` / `TypeCPort::enumerate()` /
  `PowerDeliveryPort::enumerate()` (which read `/sys` directly) are gone.
  Use `whatcable::Sysfs::linux().usb_devices()` etc., or
  `DeviceManager::new()` for the bundled aggregate.
- `whatcable::manager::DeviceManager` moved to `whatcable::DeviceManager`
  (re-exported from the new `whatcable::sysfs::manager`).
- Want to drop the libudev / clap dependency tree? Set
  `default-features = false` and pick exactly the layer you need.

## [0.2.1] - 2025

- Switch CI to `ubuntu-latest` with apt.
- Cast signal handler through `*const ()` for clippy fn-to-int lint.
- Re-enable `watch` feature by default.

[0.4.0]: https://github.com/abrauchli/usbeehive/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/abrauchli/usbeehive/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/abrauchli/usbeehive/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/abrauchli/usbeehive/releases/tag/v0.2.1
