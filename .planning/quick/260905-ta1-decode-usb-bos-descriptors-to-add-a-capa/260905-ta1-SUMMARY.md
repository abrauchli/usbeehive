---
quick_id: 260905-ta1
slug: decode-usb-bos-descriptors-to-add-a-capa
date: 2026-09-05
status: complete
commits:
  - 42633bb feat(bos) pure BOS descriptor parser + data-rate assessment
  - 3ae7a52 feat(sysfs) read bos_descriptors into UsbDevice.bos
  - 5584888 feat surface BOS capability vs negotiated data rate (additive, Devices5)
  - caf6d62 docs D-Bus consumer spec + CHANGELOG
---

# Summary — 260905-ta1

## What shipped

**`src/bos.rs`** (new, always compiled, no IO). `parse(&[u8]) ->
Option<BosDescriptors>` decodes USB 2.0 Extension (`0x02`), SuperSpeed
(`0x03`), Container ID (`0x04`), SuperSpeedPlus (`0x0A`) and Billboard
(`0x0D`). Unknown types survive as `Other { cap_type, data }`. Hostile-input
safe: never panics, never loops (`bLength < 3` terminates the walk), bounds
every read, reports partial results via `truncated`. The SuperSpeedPlus
sublink speed attribute array is fully decoded (mantissa/exponent/sublink
type/link protocol + min lane counts) so **Gen 1x2 and Gen 2x1 are
distinguishable** — both aggregate to 10 Gbps and both report `speed == 10000`.

**`DataRateAssessment::evaluate(negotiated_mbps, &BosDescriptors)`** →
`AtCapability` / `BelowCapability` / `Degraded` / `Unknown`. `is_warning` is
`true` only for `Degraded`, which requires the negotiated speed to be below
the vendor's own `bFunctionalitySupport` floor.

**sysfs.** `reader::read_bytes` (first binary reader, size-capped, `None` on
missing/empty/oversized). `UsbDevice.bos: Option<BosDescriptors>` and
`UsbDevice.quirks: u32`, plus `bos_suppressed_by_quirk()` and `data_rate()`.

**Surface.** `DeviceSummary.data_rate`, 13 additive `properties` keys, two
additive D-Bus signals, `SnapshotDiff.newly_rate_degraded` / `rate_restored`,
CLI labels, `--json` additions.

## Interface-additivity decision

**Stays `org.usbeehive.Devices5`.** No tuple shape was touched. Everything
rides the two channels the interface documents as non-breaking: extra keys in
the per-entry `properties` `a(ss)` bag, and new signals (the 0.10.x
`DeviceChanged` precedent). No bump was needed, so none was proposed.

## Data-rate vs charging split

Kept strictly separate. `Bottleneck` / `ChargingDiagnostic` /
`CapabilityDegraded (iss)` are power-side and keyed on Type-C **port
numbers**; `newly_degraded` carries `Vec<i32>`. A data-rate shortfall lands on
a plain USB device several hops behind a hub, which has no port number — there
is literally no `i` to send. So:

- new `DataRateVerdict` / `DataRateAssessment` vocabulary, not new `Bottleneck`
  variants;
- `DeviceSummary.data_rate` beside `charging_diag`, not inside it;
- `newly_rate_degraded` / `rate_restored` as `Vec<String>` summary ids;
- `DataRateDegraded (sss)` / `DataRateRestored (s)` keyed on the id string.

`usb_link_verdict` also joins the curated state fingerprint, so a verdict flip
fires `DeviceChanged` and snapshot-driven consumers stay correct without
subscribing to anything new.

## Coordinator corrections folded in

1. **`USB_QUIRK_NO_BOS`** (BIT(17), verified against
   `/usr/src/linux-headers-7.0.0-30-generic/include/linux/usb/quirks.h`).
   `UsbDevice.quirks` is now read; `bos_suppressed_by_quirk()` distinguishes
   "no BOS because USB 2.0" from "no BOS because quirked"; the
   `usb_bos_suppressed` flag key surfaces it. A quirked device produces no
   verdict at all, so it can never be reported as degraded.
2. **Billboard capability** decoded and surfaced as facts only
   (`usb_altmode_svids` / `usb_altmode_state` / `usb_altmode_failure`) — no
   warning flag, no signal. Unit-tested against spec-constructed synthetic
   fixtures.

## Test results (`rtk proxy`)

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets --all-features -- -D warnings` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test --all-features` | 276 passed, 0 failed |
| `cargo test` (default) | 251 passed, 0 failed |
| `cargo test --no-default-features --features dbus` | 257 passed, 0 failed |
| `cargo test --no-default-features` | 161 passed, 0 failed |

New tests: 35 unit tests in `src/bos.rs`, 7 integration tests in
`tests/usb_enumeration.rs`, 3 in `tests/dbus_interface.rs`.

## Live sanity check

7 devices on this box publish a BOS; none parse as truncated. Zero warnings
fire — including the SuperSpeed LAN adapter behind a USB 2.0 hub, which reads
`BelowCapability` (informational) because its vendor declares full
functionality at High Speed. A naive `capable > negotiated` rule would have
raised 2 false warnings.

## Deliberately left out

- No crate version bump, no tag, no push — the orchestrator owns releases.
- No changes to `../usbee`.
- No warning flag or signal for Billboard alt-mode state.
- `bos_descriptors` still leaks into `UsbDevice.raw_attributes` for blobs that
  happen to be valid UTF-8 (pre-existing `read_all_attrs` behaviour); not
  changed, to avoid altering `--raw` output.
