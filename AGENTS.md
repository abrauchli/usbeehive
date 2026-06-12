# usbeehive — Agent Guidelines

## Project overview

usbeehive is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable)
(macOS) by Darryl Morley — a CLI tool, and a Rust library, that show USB
device and USB-C cable information by reading Linux sysfs.

This repository is a Rust rewrite of the original C++ / CMake
implementation, organised as a single Cargo crate with feature-gated
layers.

> Previously published on crates.io as `whatcable`; renamed to
> `usbeehive` at the original author's request. See CHANGELOG entry
> for 0.5.0.

## Layout

```
usbeehive/
├── Cargo.toml                          # one package
├── src/
│   ├── lib.rs                          # public re-exports + module roots
│   ├── main.rs                         # CLI entry (gated on `cli` feature)
│   ├── output.rs                       # CLI text / JSON rendering
│   ├── pd.rs                           # USB-PD VDO decoders        ← always
│   ├── usb.rs                          # UsbDevice / UsbInterface   ← always
│   ├── typec.rs                        # TypeCPort / Cable / …      ← always
│   ├── power.rs                        # PowerDataObject / Port     ← always
│   ├── cable.rs                        # CableInfo                  ← always
│   ├── diagnostic.rs                   # ChargingDiagnostic         ← always
│   ├── summary.rs                      # DeviceSummary              ← always
│   ├── usbclass.rs / vendor.rs         # lookup tables              ← always
│   ├── sysfs/                          ← #[cfg(feature = "sysfs")]
│   │   ├── mod.rs / reader.rs          # Sysfs handle + read helpers
│   │   ├── usb.rs / typec.rs / power.rs # impl Sysfs::*
│   │   ├── manager.rs                  # DeviceManager + Snapshot
│   │   └── error.rs                    # Error / Result
│   ├── watch.rs                        ← #[cfg(feature = "watch")]
│   ├── dbus.rs                         ← #[cfg(feature = "dbus")]
│   └── bin/usbeehived.rs               # D-Bus daemon  ← gated on `dbus`
├── tests/
│   ├── fixture_builder.rs              # programmatic sysfs-tree builder
│   ├── usb_enumeration.rs              # gated on `sysfs`
│   ├── typec_pd_scenarios.rs           # gated on `sysfs`
│   ├── cli_smoke.rs                    # gated on `cli`
│   └── dbus_interface.rs               # gated on `dbus`
├── examples/{decode_cable_vdo,cable_info,list_devices,snapshot_diff,print_changes,dbus_client}.rs
├── CHANGELOG.md / README.md / AGENTS.md
└── .github/workflows/{ci,release}.yml
```

## Features

| Feature | Default | Pulls in | Adds |
|---|---|---|---|
| (none) | always | `serde` | Pure types + decoders + diagnostics |
| `sysfs` | yes | `std::fs` | `/sys` enumeration, `Sysfs`, `DeviceManager` |
| `watch` | yes | `udev`, `libc` | libudev hotplug (`Watcher`, `run_loop`) — implies `sysfs` |
| `cli` | yes | `clap`, `serde_json` | `usbeehive` binary — implies `sysfs` |
| `dbus` | no | `zbus`, `serde_json` | `usbeehive::dbus` module + `usbeehived` daemon publishing `org.usbeehive.Devices5` — implies `watch` |

`watch` and `cli` both transitively enable `sysfs`; `dbus` enables `watch`
(and therefore `sysfs`). Pure-decoder consumers go
`default-features = false` and get a `serde`-only build.

## Code conventions

- Rust 2021 edition, MSRV 1.85 (driven by transitive `clap_lex` requiring
  edition 2024). Crate-level `#![warn(missing_docs)]`,
  `#![deny(unsafe_code)]` (the watch module re-allows unsafe locally to
  call `libc::poll` / `libc::signal`).
- All sysfs reads go through `crate::sysfs::reader` — never read `/sys/`
  directly with raw `std::fs` from outside that module.
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
cargo build --release                                          # default (cli + sysfs + watch)
cargo build --release --no-default-features --features cli,sysfs    # no libudev
cargo build --release --no-default-features                    # library, pure decoders only
```

## Testing

```bash
cargo test                          # full suite, requires libudev-dev (98 tests)
cargo test --no-default-features    # decoders only, no libudev (50 tests)
```

Manual smoke tests:

- `./target/debug/usbeehive`
- `./target/debug/usbeehive --json`
- `./target/debug/usbeehive --watch`     (requires `watch` feature)
- `./target/debug/usbeehive --sysfs-root tests/fixture-root`

## Adding a new sysfs scenario

1. Build the desired tree with `tests/fixture_builder.rs` helpers
   (`UsbDeviceFixture`, `write_typec_port`, `write_typec_cable`,
   `write_pd_port`).
2. Construct a `DeviceManager::with_sysfs(Sysfs::with_root(path))`.
3. Assert against `mgr.snapshot()` — `usb_devices`, `typec_ports`,
   `pd_ports`, or the rendered `summaries`.

## Key files

| File | Purpose |
|---|---|
| `src/pd.rs` | USB-PD VDO bit-field decoders |
| `src/diagnostic.rs` | Charging-bottleneck classifier |
| `src/summary.rs` | Plain-English `DeviceSummary` |
| `src/sysfs/reader.rs` | `Sysfs` handle + `read_attr` / `read_int` / `read_hex` |
| `src/sysfs/manager.rs` | `DeviceManager` + `Snapshot` |
| `src/watch.rs` | `Watcher` + `run_loop` |
| `src/dbus.rs` | `org.usbeehive.Devices5` interface + `DeviceEntry` / `PdoEntry` / `DiagnosticEntry` wire types |
| `src/bin/usbeehived.rs` | D-Bus daemon entrypoint (zbus + hot-plug thread) |
| `src/output.rs` | CLI text + JSON rendering |
| `src/main.rs` | CLI parser + dispatch |
| `tests/fixture_builder.rs` | Programmatic sysfs-tree builder |
