# WhatCable — Agent Guidelines

## Project overview

WhatCable is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable)
(macOS) by Darryl Morley — a CLI tool that shows USB device and USB-C
cable information by reading Linux sysfs.

This repository is a Rust rewrite of the original C++/CMake implementation,
organised as a Cargo workspace.

## Workspace layout

```
whatcable/
├── Cargo.toml                       # virtual workspace manifest
├── crates/
│   ├── whatcable-core/              # pure types + USB-PD decoders, no IO
│   │   ├── src/{cable,diagnostic,pd,power,summary,typec,usb,usbclass,vendor}.rs
│   │   └── examples/{decode_cable_vdo,cable_info}.rs
│   ├── whatcable-sysfs/             # /sys backend with injectable root
│   │   ├── src/{error,manager,power,sysfs,typec,usb}.rs
│   │   ├── tests/{fixture_builder,usb_enumeration,typec_pd_scenarios}.rs
│   │   └── examples/{list_devices,snapshot_diff}.rs
│   ├── whatcable-watch/             # libudev hotplug
│   │   ├── src/lib.rs               # Watcher + run_loop + WaitResult
│   │   └── examples/print_changes.rs
│   └── whatcable-cli/               # the `whatcable` binary
│       ├── src/{main,output}.rs
│       └── tests/cli_smoke.rs
├── README.md / AGENTS.md
└── .github/workflows/release.yml
```

## Architecture

The crates form a one-way dependency chain:

```
whatcable-core ←──── whatcable-sysfs ←──── whatcable-cli
                                       ↘
                                     whatcable-watch (independent)
```

- **`whatcable-core`** is IO-free. Forbids `unsafe` (`#![forbid(unsafe_code)]`)
  and warns on missing docs. Holds the data types every backend produces
  and the PD VDO decoders. Adding a non-Linux backend (BSD-equivalent,
  remote inventory, parser for `lsusb -t` output) means: new crate that
  produces `UsbDevice` / `TypeCPort` / etc.
- **`whatcable-sysfs`** owns all `std::fs` reads under `/sys`. The root
  path is injected via `Sysfs::with_root()` — *no module reads `/sys/...`
  directly*, so fixture trees swap in cleanly.
- **`whatcable-watch`** is a thin wrapper over the `udev` crate, plus the
  signal-handler / debounce loop the CLI used to embed.
- **`whatcable-cli`** is the binary + JSON / text output. Library
  consumers should never depend on this crate.

## Key data flow (sysfs runtime)

```
sysfs::Sysfs (root path)
  → /sys/bus/usb/devices/         → sysfs::usb::enumerate_in
  → /sys/class/typec/             → sysfs::typec::enumerate_in
  → /sys/class/usb_power_delivery → sysfs::power::enumerate_in
                                       ↓
                              sysfs::manager::DeviceManager
                                       ↓
                              core::summary::DeviceSummary
                                       ↓
                              cli::output::print_text / print_json
```

## Code conventions

- Rust 2021, MSRV 1.74. Workspace-level dependencies live in the root
  `Cargo.toml`'s `[workspace.dependencies]` table.
- All sysfs reads go through `whatcable_sysfs::sysfs` — never read `/sys/`
  directly with raw `std::fs` from anywhere else in the workspace.
- Handle missing sysfs paths gracefully — return `None` / empty
  collections, never panic. Many systems lack `/sys/class/typec/` or
  `/sys/class/usb_power_delivery/`.
- Identity VDOs are pushed in **USB-PD spec order** (`id_header`,
  `cert_stat`, `product`, `product_type_vdo1..3`), not alphabetical filename
  order. Decoders rely on this — `vdos[0]` is the ID Header,
  `vdos[3]` is the Cable VDO.
- Source files derived from the original Swift code keep the attribution
  header noting the WhatCable / Zetaphor port lineage where applicable.
- Prefer `Option<T>` and `Result<T, E>` over sentinel values; prefer
  iterator chains over manual loops where the chain is clearer.
- Use `serde` derive for any type that may end up in `--json` output.

## Build

```bash
cargo build --workspace --release                          # default (with --watch)
cargo build -p whatcable-cli --release --no-default-features    # no libudev
```

## Testing

```bash
cargo test --workspace --no-default-features    # no libudev required
cargo test --workspace                          # full, requires libudev-dev
```

Manual smoke tests:

- `./target/debug/whatcable`
- `./target/debug/whatcable --json`
- `./target/debug/whatcable --watch`     (requires `watch` feature)
- `./target/debug/whatcable --sysfs-root tests/fixture-root`     (against a captured tree)

## Adding a new sysfs backend or test scenario

1. Build the desired tree with `tests/fixture_builder.rs` helpers
   (`UsbDeviceFixture`, `write_typec_port`, `write_typec_cable`,
   `write_pd_port`).
2. Construct a `DeviceManager::with_sysfs(Sysfs::with_root(path))`.
3. Assert against `mgr.snapshot()` — `usb_devices`, `typec_ports`,
   `pd_ports`, or the rendered `summaries`.

## Key files to know

| File | Purpose |
|---|---|
| `crates/whatcable-core/src/pd.rs` | USB-PD VDO bit-field decoders |
| `crates/whatcable-core/src/diagnostic.rs` | Charging-bottleneck classifier |
| `crates/whatcable-core/src/summary.rs` | Plain-English `DeviceSummary` |
| `crates/whatcable-sysfs/src/sysfs.rs` | `Sysfs` handle + read helpers |
| `crates/whatcable-sysfs/src/manager.rs` | `DeviceManager` + `Snapshot` |
| `crates/whatcable-sysfs/tests/fixture_builder.rs` | Programmatic sysfs-tree builder for tests |
| `crates/whatcable-watch/src/lib.rs` | `Watcher` + `run_loop` |
| `crates/whatcable-cli/src/output.rs` | Text + JSON rendering |
| `crates/whatcable-cli/src/main.rs` | CLI parser + dispatch |
