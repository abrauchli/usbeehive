# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-05-02

Major restructure: the project is now a Cargo workspace of four
independently-published crates, with a real library API and a fixture-driven
test suite.

### Added

- **`whatcable-core`** crate (new) — IO-free types + USB-PD VDO decoders +
  diagnostics + summaries. Forbids `unsafe`, warns on missing docs.
- **`whatcable-sysfs`** crate (new) — Linux `/sys` enumeration backend with
  an injectable root path. `Sysfs::with_root()` lets tests/fixtures swap
  `/sys` for a captured tree on disk; `Sysfs::try_with_root()` validates
  existence. `DeviceManager::with_sysfs()` lets consumers pick the root.
  Public `Snapshot` struct exposes the structured `usb_devices`,
  `typec_ports`, `pd_ports`, and `summaries` together. New `Error` /
  `Result` types in `error.rs`.
- **`whatcable-watch`** crate (new) — libudev hotplug monitor. `Watcher`
  for low-level `poll(2)` integration; `run_loop()` for a debounced
  render loop with built-in `SIGINT` / `SIGTERM` handling.
- **`whatcable`** binary — new `--sysfs-root <PATH>` flag for running
  against captured fixture trees.
- `TypeCPowerSupply::negotiated_power_mw()` — `i128`-safe live wattage
  helper, deduplicated from two prior call sites in summary / JSON output.
- Five runnable `examples/` — `decode_cable_vdo`, `cable_info`,
  `list_devices`, `snapshot_diff`, `print_changes`.
- 97 tests across the workspace (up from ~30), including end-to-end CLI
  smoke tests that build a sysfs tree on disk and exercise the binary.
- New CI workflow (`.github/workflows/ci.yml`): test matrix
  (default + `--no-default-features`), `cargo fmt --check`, `cargo clippy
  --all-targets -D warnings`, MSRV (1.74) build, rustdoc with
  `-D warnings`.
- Per-crate `README.md` and crate-level doc comments on every public item.
- Workspace `CHANGELOG.md` (this file).

### Changed

- The bin crate (`whatcable`) lives at `crates/whatcable-cli/`. Library
  consumers depend on the new specialized crates above; downstream tools
  no longer have to pull in the whole binary's dependency tree.
- `ProductType` and `PdoType` use `#[derive(Default)]` + `#[default]`
  rather than hand-written `impl Default`.
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

- `cargo install whatcable` — same as before, now installs the
  workspace's bin crate at 0.3.0.
- The published `whatcable` package is no longer a library. Library code
  that depended on `whatcable::*` should switch to:
  - `whatcable_core::*` for types and decoders,
  - `whatcable_sysfs::*` for enumeration,
  - `whatcable_watch::*` for hotplug.
- `UsbDevice::enumerate()` / `TypeCPort::enumerate()` /
  `PowerDeliveryPort::enumerate()` (which read `/sys` directly) are gone.
  Use `whatcable_sysfs::Sysfs::linux().usb_devices()` etc., or
  `DeviceManager::new()` for the bundled aggregate.

## [0.2.1] - 2025

- Switch CI to `ubuntu-latest` with apt.
- Cast signal handler through `*const ()` for clippy fn-to-int lint.
- Re-enable `watch` feature by default.

[0.3.0]: https://github.com/Zetaphor/whatcable-linux/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/Zetaphor/whatcable-linux/releases/tag/v0.2.1
