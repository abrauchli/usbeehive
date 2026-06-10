---
title: Type-C port ↔ enumerated USB device correlation
status: planned
date: 2026-06-10
version_bump: none (stays 0.10.0 — unreleased)
interface_bump: none (stays org.usbeehive.Devices5 — additions only)
---

# Quick Task: Port↔USB correlation, USB-string partner ID, negotiated-speed cable check

## Decisions (locked by user)

- Correlate each Type-C port with its enumerated USB device through the
  partner directory's USB **child node** (a `bus-port` subdir like `2-2`) —
  the kernel's canonical port↔USB-device linkage, mirroring the
  `usb_power_delivery` symlink pairing shipped this morning.
- The Devices5 interface is **unreleased** — if anything here would break the
  wire, fold it into Devices5 / 0.10.0 rather than bumping to Devices6.
  Nothing below changes the signature: the additions are one `TypeCPartner`
  field (JSON-only, behind `SnapshotJson`), one `properties` key
  (`usb_device`), a subtitle string change, and a refinement to the existing
  `cable.data_speed_limit` trigger — all non-breaking per the documented
  conventions in `src/dbus.rs`.
- Subtitle output changes (identity VDO label > USB product string >
  "Device connected") are prose, not a wire break; tests asserting the old
  subtitle are updated.
- usbee (sister repo at `/home/blk/projects/rust/usbee`) is updated in a
  separate final task — label-table entry for the new `usb_device` key +
  CHANGELOG note, no wire change.

## Live-hardware evidence (verified 2026-06-10 on the dev laptop)

Pixel 7 on port0 (laptop = host/source), 65W charger on port1:

- `/sys/class/typec/port0-partner/` contains a child dir `2-2` — the
  enumerated USB device's bus-port position. Child name pattern:
  `^\d+-\d+(\.\d+)*$` (bus dash port, optional hub-chain dots). This is the
  kernel's canonical port↔USB-device linkage.
- The partner has **no `identity/`** dir (PD 2.0 phone, no Discover
  Identity) → current subtitle falls back to "Device connected".
- The USB device list carries the Pixel at `bus_port = "2-2"`: VID:PID
  `18d1:4ee7`, product string "Pixel 7", negotiated link speed SuperSpeed+
  10 Gbps (`speed` attr = `10000`), USB 3.2.
- Port0 computed summary today: `status: Sourcing`, subtitle "Device
  connected", all power fields 0 (correct — no PD data for the sourcing
  direction; do **not** invent wattage).
- Port1 (charger) enumerates no USB device → no bus-port child node in
  `port1-partner`. New code must be a silent no-op there.

## Scope (3 atomic tasks — one commit each; usbee is its own commit)

### Task 1 — Parse the partner's USB child node + correlate in build_summaries

**Files**: `src/typec.rs`, `src/sysfs/typec.rs`, `src/sysfs/manager.rs`,
`tests/fixture_builder.rs`

1. `src/typec.rs` — `TypeCPartner` gains `usb_name: String` (empty when no
   USB child node is present; plain String, not Option, to keep `Default`
   derivable — match the `pd_name` field added this morning, immediately
   after it). Doc: "Basename of the partner's enumerated USB device child
   directory (a `bus-port` like `"2-2"`) — the kernel's canonical linkage
   from a Type-C partner to its USB device node. Empty when the partner
   enumerates no USB device (e.g. a charger)."

2. `src/sysfs/typec.rs` — add a `partner_usb_name(partner_path: &Path) ->
   String` helper modeled on the existing `partner_pd_name` (lines 142–158):
   scan `reader::subdirs(partner_path)` for the first (sorted) child whose
   `file_name()` matches `^\d+-\d+(\.\d+)*$` — i.e. splits on `-` into a
   numeric bus and a port segment, where the port segment is `\d+` optionally
   followed by `.\d+` hub-chain groups. Implement the match with byte checks
   (no regex crate — the codebase uses manual ASCII checks, see
   `partner_pd_name`). Return empty on no match. Wire it into the
   `TypeCPartner { … }` construction in `from_sysfs` (~line 230) as
   `usb_name: partner_usb_name(&partner_path),`, right after the existing
   `pd_name:` line.

3. `src/sysfs/manager.rs::build_summaries` (~line 222) — after the existing
   `pd_match` block and before `DeviceSummary::from_typec_port`, resolve the
   correlated USB device:
   `let usb_match = tc.partner.as_ref().filter(|pa|
   !pa.usb_name.is_empty()).and_then(|pa| usb.iter().find(|d|
   d.bus_port == pa.usb_name));`
   Pass `usb_match` (as `Option<&UsbDevice>`) into `from_typec_port` — see
   Task 2 for the new parameter. Add a comment in the manager's existing
   comment style: the partner's USB child dir is the kernel's canonical
   port↔USB-device linkage (same shape as the `usb_power_delivery` symlink
   pairing); chargers enumerate no USB device so `usb_name` is empty there
   and the lookup is a no-op.

4. `tests/fixture_builder.rs` — add a helper modeled on `link_partner_pd`
   (lines 246–254):
   `pub fn link_partner_usb(root: &Path, port_name: &str, bus_port: &str)`
   that creates `<root>/class/typec/<port_name>-partner/<bus_port>/` as a real
   subdirectory (kernel exposes it as a child dir, not a symlink — a plain
   `fs::create_dir_all` of the partner dir joined with `bus_port`). Doc it as
   the USB-device child node the partner parse scans for.

5. Tests (`src/sysfs/typec.rs` unit tests): partner dir with a `2-2` subdir →
   `partner_usb_name` returns `"2-2"`; partner dir with a `pd1` subdir and no
   bus-port child → returns `""` (must not match the `pdN` node); hub-chain
   name `2-2.1.4` → returned verbatim; non-matching names (`identity`,
   `port0-partner.0`, `foo`) → `""`.
   Manager unit test (`src/sysfs/manager.rs` tests): one port with a partner
   carrying `usb_name = "2-2"` + a `UsbDevice { bus_port: "2-2", product:
   "Pixel 7", speed: 10000, .. }` in the list → the port summary correlates
   (assert via the subtitle from Task 2 and/or the `usb_device` property);
   a second port whose partner has empty `usb_name` (charger) → no
   correlation, no `usb_device` property (regression for the no-op).

**Verification**: `cargo test --all-features` (new parse + manager tests),
`cargo clippy --all-features -- -D warnings`.

### Task 2 — USB-string partner subtitle fallback + `usb_device` cross-ref property

**Files**: `src/summary.rs`, `src/sysfs/manager.rs` (call-site only),
`src/dbus.rs` (doc note), `tests/dbus_interface.rs` (assert only, if a
fixture there asserts the old port0 subtitle)

1. `src/summary.rs::from_typec_port` (~line 671) — add a fourth parameter
   `usb: Option<&UsbDevice>` (after `cable_info`). Crate-internal signature
   change, fine pre-release. Update the manager call site from Task 1 to pass
   `usb_match`.

2. Subtitle fallback chain (`from_typec_port`, the partner block at
   lines 706–724). Precedence, highest first:
   1. **Identity VDO label** — the existing branch (vendor — product_type, or
      product_type alone). Keep exactly as-is; identity wins when present.
   2. **USB product string** — when the partner has no usable identity VDO
      (the current `else` branch) AND `usb` is `Some(d)` with a non-empty
      `d.product`: set `s.subtitle = d.product.clone()` (e.g. "Pixel 7"). If
      `d.product` is empty but `d.manufacturer` is non-empty, use that;
      prefer `"{manufacturer} {product}"` only when both are present and
      non-empty (mirror the `vendor — product` shape but with a space, since
      USB strings are free-form). Do **not** fabricate from VID:PID here.
   3. **"Device connected"** — final fallback when neither identity nor a USB
      string is available (charger-with-partner, or a USB device with empty
      descriptor strings).
   Keep the existing comment style; note that identity VDOs keep precedence
   and the USB string is the no-identity enrichment path.

3. Cross-reference property (`from_typec_port`, alongside the other
   `s.properties.push` calls — put it just after the `data_role` /
   `power_mode` pushes, before the PD/cable blocks). When `usb` is `Some(d)`:
   `s.properties.push(("usb_device".into(), format!("usb:{}", d.bus_port)));`
   Use the `usb:<bus_port>` **stable-id form** (matches the daemon id
   convention documented in `src/dbus.rs` line 32: `usb:<bus_port>`), so a UI
   can resolve the linked entry by id. Document the chosen form inline.
   Reverse link: skip — the USB entry is built separately in
   `from_usb_device` without port context, so a reverse `typec_port` property
   is not cheap here; leave it out (note this in the comment).

4. `src/dbus.rs` — in the `properties` additive-keys notes (the doc table /
   prose around line 45 and the enum-extensibility section), add a bullet
   recording the new additive key: `usb_device` = `usb:<bus_port>` of the
   enumerated USB device correlated to this Type-C port via the partner's USB
   child node; present only on `TypeCPort` entries whose partner enumerated a
   USB device; adding it is non-breaking. Also note the new JSON-only
   `TypeCPartner.usb_name` field surfaced through `SnapshotJson` (additive,
   serde default-empty).

5. `src/output.rs` — add `"usb_device" => "USB device".into(),` to
   `property_label` (value-bearing key; renders as `USB device: usb:2-2`).
   Not a flag, not yellow — it is a neutral cross-reference, plain style.

6. Tests:
   - `src/summary.rs` tests: partner with **no identity** + USB device
     `product = "Pixel 7"` → subtitle == "Pixel 7" and a
     `("usb_device", "usb:2-2")` property present; partner **with** identity
     VDO (Hub) + a USB device → subtitle stays the identity label (identity
     wins), property still present; partner with empty `usb_name` / `usb =
     None` → subtitle "Device connected", no `usb_device` property.
   - Update any assertion in `tests/dbus_interface.rs` that expects the old
     "Device connected" subtitle for a fixture that now supplies a USB
     product string.

**Verification**: `cargo test --all-features`, then live
`cargo run --bin usbeehive -- --list` — port0 subtitle reads "Pixel 7" and a
"USB device: usb:2-2" row appears; port1 (charger) unchanged.

### Task 3 — Strengthen `cable.data_speed_limit` with the negotiated link speed

**Files**: `src/summary.rs`, `tests/` (covered inline in summary.rs tests)

Context: the existing check (summary.rs lines 811–838) emits
`cable.data_speed_limit` when a Hub/Peripheral partner's UFP-VDO advertised
speed exceeds the e-marked cable speed. The correlation now also gives the
**negotiated** USB link speed (`usb.speed`, Mbps). Use it as an additional,
conservative trigger/refinement — keep the VDO-based path intact.

1. `src/summary.rs` — add a small mapper, near the cross-check block:
   `fn negotiated_cable_speed(mbps: u32) -> Option<CableSpeed>` mapping the
   negotiated Mbps to the comparable `CableSpeed` tier (reuse the thresholds
   that `cable_speed_max_gbps` encodes; do not invent new ones):
   `>= 40000 → Usb4Gen4? ` — be precise: `< 5000 → Usb20`,
   `5000..<10000 → Usb32Gen1`, `10000..<20000 → Usb32Gen2`,
   `20000..<40000 → Usb4Gen3`, `>= 40000 → Usb4Gen4`. Return `None` for
   `0` (unknown — never trigger on unknown speed). Add a doc comment noting
   these mirror `cable_speed_max_gbps`'s single-lane Gbps buckets.

2. Extend the existing cross-check block (lines 816–838) so `from_typec_port`
   takes the correlated `usb` device's negotiated speed into account. Pass
   the device's `speed` in (it is already available as the new `usb`
   parameter from Task 2). New conservative trigger, **in addition** to the
   VDO comparison:
   - Compute `device_capability` = the **partner's advertised** capability:
     the UFP-VDO highest speed (already decoded as `partner_speed` in the
     existing block). Keep using it as the "what the device claims it can do"
     figure.
   - Compute `negotiated = negotiated_cable_speed(usb.speed)` (None when no
     correlated device or speed 0).
   - Emit `cable.data_speed_limit = cable_speed_label(cable_speed)` when
     EITHER:
     (a) the **existing** VDO trigger fires (`partner_speed.is_some_and(|ps|
     cable_speed < ps)`), OR
     (b) the **negotiated** trigger fires: `partner_speed` is `Some(ps)` AND
     `cable_speed < ps` AND `negotiated.is_some_and(|n| n <= cable_speed)` —
     i.e. the device advertises faster than the cable, the cable is e-marked
     at/below what the link actually negotiated, so the cable is observed
     evidence of the limit (the link did not exceed the cable's rating).
   In practice (b) is a confidence refinement of (a): both require
   `cable_speed < ps`. The extra `negotiated <= cable_speed` clause is what
   guards against false positives. Emit the property **once** (dedupe — do
   not push twice if both fire).
   - **Do not** emit when `partner_speed` is `None` (no UFP VDO) and we only
     have a negotiated speed: a negotiated speed legitimately equals the
     slower of device/host-port capabilities, so without the partner's
     *advertised* capability there is no evidence the cable is implicated.
     Negotiated-only is NOT a trigger. (This keeps the live Pixel-7 case — no
     identity VDO — from spuriously flagging the cable.)
   Keep the comment explaining: negotiated speed alone is ambiguous (host or
   device may be the limit); the cable is only implicated when the partner
   *advertises* more than the cable is rated for AND the link did not exceed
   the cable's rating.

3. Tests (`src/summary.rs`): 
   - VDO Gen2 partner + USB 2.0 cable + negotiated 480 Mbps device →
     property present, value "USB 2.0" (both triggers agree).
   - VDO Gen2 partner + Gen2 cable + negotiated 10000 → absent (cable not
     below capability).
   - **No-VDO partner** (PD 2.0 phone, `partner_speed` None) + USB 2.0 cable
     + negotiated 480 → **absent** (negotiated-only must not trigger — the
     live Pixel-7-style guard).
   - VDO Gen2 partner + USB 2.0 cable but negotiated 10000 Mbps (link
     exceeded the e-marker rating — inconsistent data) → trigger (b) does NOT
     fire (`negotiated <= cable_speed` is false), but trigger (a) still does;
     assert present via the VDO path, confirming the refinement never
     *suppresses* the existing behaviour.

**Verification**: `cargo test --all-features`, `cargo fmt --check`,
`cargo clippy --all-features -- -D warnings`.

### Task 4 — usbee label-table + CHANGELOG (sister repo, separate commit)

**Files** (in `/home/blk/projects/rust/usbee`):
`usbee@bitcreed.us/src/label-table.js`, `po/usbee@bitcreed.us.pot`,
`CHANGELOG.md`

1. `src/label-table.js` — add `['usb_device', _('USB device')],` to the
   LABEL_TABLE, in the additive-keys group (near `cable.data_speed_limit`,
   lines 48–50). The value (`usb:2-2`) renders verbatim — no UNIT_BY_KEY or
   flagValueTable entry needed (it is a plain value-bearing key).
   Decision (documented): do **not** add `usb_device` to
   `HANDLED_BY_DEDICATED_UI` in `popover.js`. usbee has no dedicated
   port↔device link widget yet; rendering the cross-ref as a normal row
   (`USB device: usb:2-2`) is the useful behaviour. Revisit only if/when a
   click-through UI lands.

2. `po/usbee@bitcreed.us.pot` — add the `usb_device` → "USB device" msgid by
   hand (xgettext is not installed on this machine — mirror the existing
   hand-edited entries' format).

3. `CHANGELOG.md` — under the existing unreleased `[2.4.0]` entry, add an
   "Added" bullet: daemon now correlates Type-C ports with their enumerated
   USB device (via the partner's USB child node) and emits an additive
   `usb_device` property (`usb:<bus_port>`); ports whose partner has no PD
   identity now show the USB product string ("Pixel 7") instead of "Device
   connected"; no wire change, `MIN_USBEEHIVE_VERSION` stays 0.10.0.

**Verification**: `cd /home/blk/projects/rust/usbee && node -c
usbee@bitcreed.us/src/label-table.js` (syntax check); visual diff of the
CHANGELOG and pot entries.

## Out of scope

- A reverse `typec_port` property on the USB-device entry — `from_usb_device`
  has no port context and threading it is not cheap; the forward `usb_device`
  cross-ref is sufficient for a UI to link both directions.
- Inferring wattage for the sourcing direction (port0) — no PD data exists
  there; the subtitle/correlation changes add the USB identity only, never
  fabricated power.
- A new D-Bus interface — every change is additive to Devices5 / 0.10.0.
- usbee dedicated port↔device link UI (deferred; the cross-ref renders as a
  plain row for now).

## CHANGELOG (usbeehive)

Fold into the existing unreleased `[0.10.0]` entry: new "Added" bullets for
the partner USB-child-node correlation (`TypeCPartner.usb_name` +
`usb_device` property), the USB-product-string subtitle fallback chain
(identity VDO > USB product > "Device connected"), and the negotiated-speed
refinement of `cable.data_speed_limit`. Do not create a new version heading.
