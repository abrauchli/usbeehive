# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-05-02

Metadata-only patch release.

### Changed

- `repository` and `homepage` in `Cargo.toml` now point to
  `github.com/abrauchli/whatcable` (the active fork) instead of the
  upstream Zetaphor repo.
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

[0.3.1]: https://github.com/abrauchli/whatcable/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/abrauchli/whatcable/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/Zetaphor/whatcable-linux/releases/tag/v0.2.1
