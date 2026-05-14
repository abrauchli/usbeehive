---
slug: devices2-structured-wire
date: 2026-05-13
status: in-progress
---

# PLAN — Devices2 structured wire

CONTEXT.md is authoritative. This plan is execution order.

## Stage A — `DeviceClass` enum + classifier

**Files:** `src/usb.rs` (or new `src/device_class.rs`).

- Define `DeviceClass` enum with 19 variants per CONTEXT.md.
- `DeviceClass::classify(&UsbDevice) -> DeviceClass` reads:
  - `is_hub` → `Hub`
  - `vendor_id` + lower-case `product` for `Phone`, `SecurityKey`
  - interface class/subclass/protocol scan for HID
    (`Keyboard`/`Mouse`/`Gamepad`/`InputTablet`), `Storage`,
    `Audio`, `Camera`/`VideoCapture`, `Printer`, `SmartcardReader`,
    `Bluetooth`, `Serial`, `NetworkWired`, `NetworkWireless`
  - `primary_driver` allowlist for `Serial`
  - fallthrough → `Unknown`
- `classify_subclass(&UsbDevice, DeviceClass) -> &'static str` returns
  the day-one subclass string (`""` for most).

**Tests in this stage:** unit tests in `src/usb.rs` for the classifier
against synthetic `UsbDevice` fixtures (HID/keyboard, HID/mouse, CCID
smartcard, FTDI serial, mass storage, hub, unknown). Covers ~80% of
the matrix without sysfs.

## Stage B — `DeviceSummary` structural refactor

**Files:** `src/summary.rs`, `src/lib.rs`.

- Drop `bullets: Vec<String>`.
- Add `properties: Vec<(String, String)>` (machine keys per CONTEXT).
- Add structured fields:
  ```rust
  pub device_class: DeviceClass,
  pub device_subclass: String,
  pub link_speed_mbps: u32,
  pub usb_version: String,        // canonical short form, no "USB " prefix
  pub power: PowerSummary,
  pub primary_driver: String,
  pub vendor: String,
  pub product: String,
  pub vendor_id: u16,
  pub product_id: u16,
  ```
- New `PowerSummary` struct (in `src/summary.rs` or `src/power.rs`):
  ```rust
  pub struct PowerSummary {
      pub power_in_mw: u32,
      pub power_out_mw: u32,
      pub power_role: PowerRole,
  }
  pub enum PowerRole { Source, Sink, DualRole, Unknown }
  ```
- Add `Status::Sourcing` variant.
- Refactor `from_usb_device` / `from_typec_port` to populate the new
  fields. Move the bullet-formatting code out — properties become
  `("serial", "ABC123")` / `("usb_power_ma", "500")` / etc.

**Property generation rules:**
- `serial` — when `dev.serial` non-empty
- `mount` — `"removable"` / `"fixed"` (raw value, not "Removable"/"Built-in")
- `drivers` — when 2+ unique drivers across interfaces; comma-joined
- `usb_power_ma` — `dev.max_power_ma.to_string()` when non-zero, USB
  devices only
- `data_role` — `port.current_data_role()` when non-empty
- `power_mode` — `port.power_op_mode` when non-empty
- `pd_revision` — `port.pd_revision` when non-empty
- `plug_orientation` — when non-empty and != `"unknown"`
- `pd_contract` — formatted from UCSI live readout when contracted
- `cable_speed` — `cable_speed_label(speed)` when present
- `cable_current` — `cable_current_label(rating)` when present
- `cable_max_power` — `format!("{}W", max_watts)` when non-zero
- `cable_type` — `"active"` / `"passive"` from CableInfo flags
- `cable_vendor` — when non-empty and not hex-fallback
- `charger_max` — `format!("{}W", max_w)` when PD source present

**Tests:** migrate existing `src/summary.rs::tests` to assert
`properties`, not `bullets`. Add tests for the new structured fields.

## Stage C — CLI `src/output.rs` rewrite

**Files:** `src/output.rs`.

- New `fn property_label(key: &str) -> &'static str` — machine key
  → English label table.
- Text renderer (`print_text_iter`): iterate `dev.properties`,
  render each as `${property_label(k)}: ${v}`. Same `• ` bullet
  visual.
- JSON (`device_json`):
  - Drop `"bullets"` key.
  - Add `"properties"` as an array of `[key, value]` arrays (matches
    D-Bus a(ss) shape).
  - Add new top-level fields: `deviceClass`, `deviceSubclass`,
    `vendor`, `product`, `vendorId` (hex), `productId` (hex),
    `primaryDriver`, `linkSpeedMbps`, `usbVersion`, `power` object,
    `status` string.
  - Keep existing nested `usb` / `typec` / `cable` / `powerDelivery`
    structures for backward compat where they're not duplicated by
    the new top-level fields.

**Tests:** update `src/output.rs::tests` for the new JSON keys.

## Stage D — D-Bus interface bump to Devices2

**Files:** `src/dbus.rs`, `src/bin/usbeehived.rs`.

- New `DeviceEntry` mirror with 19 fields per CONTEXT.md wire signature.
- New `PowerEntry` wire struct: `{ power_in_mw: u32, power_out_mw: u32, power_role: String }`.
- `DiagnosticEntry` keeps the existing `(bsssb)` shape (already
  correct — restored after the v3 USBee reversal).
- `impl From<&DeviceSummary> for DeviceEntry` — straight copy of
  structured fields, format enums via `format!("{:?}", …)`.
- `#[interface(name = "org.usbeehive.Devices2")]` bump.
- Module docs (`//!` block) — update wire-surface table to 19-field
  shape; "Migrating from Devices1" section with regex → field
  pointers (cross-links to CHANGELOG).
- `src/bin/usbeehived.rs` — update `gdbus call` doc example to
  `Devices2.ListDevices`.

**Tests:** rewrite `tests/dbus_interface.rs`:
- Migrate existing tests to new shape.
- Add: `list_devices_carries_full_structured_fields` (every top-level
  field non-default on cable-limit fixture).
- Add: `usb_device_only_fixture` (no Type-C, USB fields populated).
- Add: `empty_typec_port` (empty subtitle "Nothing connected",
  power_role Unknown).

## Stage E — README / CHANGELOG / examples

**Files:** `README.md`, `CHANGELOG.md`, `examples/dbus_client.rs`,
`Cargo.toml`.

- `Cargo.toml` feature comment: `Devices1` → `Devices2`.
- `examples/dbus_client.rs` — bump proxy to `Devices2`. Print every
  top-level field per entry plus `Diagnose(0)`.
- `README.md` D-Bus wire-surface table — rewrite for 19-field shape.
- `CHANGELOG.md [Unreleased]` — replace the existing icon entry's
  Devices1 references; add the full Devices2 breaking-change entry
  with:
  - Three-surface scope (lib + CLI JSON + D-Bus)
  - Full regex → field table
  - Bullet-label → property-key table
  - Enum-extensibility convention
  - USBee migration pointer

## Stage F — Verify + commit

- `cargo build --all-features`
- `cargo build --no-default-features`
- `cargo test --all-features`
- `cargo test --no-default-features`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- Write SUMMARY.md.
- One atomic commit covering all stages. `src/summary.rs`'s
  pre-existing icon work (already on disk, documented in CHANGELOG)
  rolls in as part of the larger refactor — the icon-detection
  changes there are subsumed by the new `DeviceClass` classifier.

## Risks

- **Breaking serde-derive on `DeviceSummary`.** The existing JSON
  consumers (`usbeehive --json`) may rely on field order or specific
  serializations. Mitigation: explicit field-by-field test in
  `output.rs::tests`.
- **Mass test churn.** Every fixture-driven test that asserts
  `bullets.iter().any(|b| b == "...")` needs migration. Audit list:
  - `src/summary.rs` (5 tests)
  - `tests/typec_pd_scenarios.rs` (likely 5-10 assertions)
  - `tests/usb_enumeration.rs` (likely 3-5 assertions)
  - `tests/dbus_interface.rs` (full rewrite)
  - `src/output.rs::tests` (4 tests)
- **`PowerSummary` field types.** D-Bus `u` (uint32) vs Rust `u32`
  alignment — already correct. `power.power_role` enum → string via
  `format!("{:?}", …)` matches the convention.
- **Pre-existing `M src/summary.rs`** changes (icon updates the user
  staged before this session). They overlap with the refactor —
  resolution: rebase those changes into the new `DeviceClass`
  classifier so nothing is lost.
