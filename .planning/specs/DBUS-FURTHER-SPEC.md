# DBUS-FURTHER-SPEC — additive `org.usbeehive.Devices5` surface for FURTHER-WINS findings

> ## IMPLEMENTATION STATUS — read this first
>
> This document is the **full survey proposal**, not the shipped contract.
> Only a deliberately trimmed subset of it was implemented. The daemon does
> **not** emit the deferred keys below; do not write a client against them.
>
> **The shipped, client-facing contract is
> `.planning/specs/DBUS-TRIM-CONSUMER-SPEC.md`.** Use that document to build
> a consumer. Use this one to decide what to ship next.
>
> | Section | Key | Status |
> |---|---|---|
> | A | `port.id`, `port.peer_id`, `port.peer_state`, `port.connect_type`, `hub.ports_total`, `hub.ports_used` | **IMPLEMENTED** (quick task 260905-tb1) |
> | A | `port.disabled`, `hub.multi_tt` | DEFERRED |
> | B | `power.source`, `hub.power_budget_ma`, `hub.power_committed_ma` | **IMPLEMENTED** |
> | B | `power.remote_wakeup_capable` | DEFERRED |
> | C | `pm.status`, `pm.autosuspend`, `pm.wakeup`, `connected_s` | DEFERRED — whole section |
> | D | `kernel.quirks` | **IMPLEMENTED** |
> | D | `kernel.avoid_reset`, `authorized` | DEFERRED |
> | E | `function.*` (net / bluetooth / input / block / sound / video) | DEFERRED — whole section |
> | F | `interface.<N>.name`, `usb.configurations` | DEFERRED — whole section |
> | G | `product_db` | **IMPLEMENTED** |
> | H | `PortOverCurrent` signal | DEFERRED |
>
> Two presence refinements were made to the implemented keys, both
> documented in the consumer spec: `port.connect_type` omits the literal
> `unknown` the kernel writes on every hub-internal port, and `product_db`
> is omitted when the hwdb name adds nothing over the device's own
> `iProduct`. Section C's fingerprint rule was honoured as written —
> `port.peer_state` is fingerprinted; `hub.ports_used` and
> `hub.power_committed_ma` were additionally kept out, since they move only
> on plug/unplug, which already fires `DeviceAdded` / `DeviceRemoved`.
>
> The deferred items remain accurate as written and were verified against
> live sysfs; nothing below needs re-research to pick up next cycle.

Companion to `FURTHER-WINS.md` (same directory). Authoritative contract:
`/home/blk/projects/rust/usbeehive/src/dbus.rs` lines 1–103. This spec is
**additive-only**: every item below is either a new `(machine_key, value)` pair in
the per-entry `properties` bag (`a(ss)`, position 14) or a new signal. Both are
documented as NON-BREAKING in `dbus.rs` (line 45: "Adding keys is non-breaking";
line 67: `DeviceChanged` precedent for additive signals). **No existing tuple shape,
method signature, enum, or position changes. No `Devices6` cut is required.**

Ownership boundary: anything derived from `bos_descriptors` (capability vs.
negotiated link speed, SuperSpeedPlus sublinks, and — per FURTHER-WINS #8 — the
Billboard capability) belongs to `.planning/specs/DBUS-BOS-CONSUMER-SPEC.md`. This
spec defines no key under a `bos.*` or `link.capable*` name and does not restate
theirs. Where a consumer would naturally combine the two, it is called out.

## Conventions (inherited from Devices5)

- Keys are lowercase `snake.dotted` machine identifiers; values are UTF-8 strings.
- Booleans are the literal strings `true` / `false`; a flag key that is `false`
  MAY simply be omitted (the existing `transport.*` keys follow the omit-when-false
  pattern; consumers already treat absence as false).
- Integers are decimal ASCII. Durations are seconds. Rates are Mbps.
- Absence of a key means "unknown / not applicable on this kernel or hardware",
  never "zero". Consumers MUST tolerate every key being absent.
- Unknown enum values MUST be treated as `Unknown` (existing convention, dbus.rs
  lines 96–102).
- Keys apply to `UsbDevice` and `Hub` category entries unless stated otherwise;
  `TypeCPort` entries carry none of them (their USB side is reachable through the
  existing `usb_device` cross-reference).

## A. Physical connector / port (FURTHER-WINS #1)

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `port.id` | basename of the hub-port object the device hangs off | device has a `port` symlink | `usb5-port2`, `5-2.1-port1` |
| `port.peer_id` | basename of the companion (other-speed) root-hub port for the same connector | `port/peer` exists (root-hub ports only) | `usb6-port2` |
| `port.peer_state` | companion port `state` verbatim: `not attached` \| `attached` \| `powered` \| `reconnecting` \| `unauthenticated` \| `default` \| `addressed` \| `configured` \| `suspended` | `port.peer_id` present and peer `state` readable | `not attached` |
| `port.connect_type` | port `connect_type` verbatim: `hotplug` \| `hardwired` \| `not used` \| `unknown` | readable | `hotplug` |
| `port.disabled` | `true` | port `disable` reads `1` | (never on this box) |
| `hub.ports_total` | `maxchild` | entry is a hub | `4` |
| `hub.ports_used` | count of the hub's `*-portN/state` values that are `configured` or `suspended` | entry is a hub and port objects readable | `2` |
| `hub.multi_tt` | `true` | hub `bDeviceProtocol == 02` (or active alt-setting proto 2) | `true` on `usb:5-2` |

Consumer note: `port.peer_state == "not attached"` on a device whose BOS-derived
capability (other agent's keys) says SuperSpeed-capable is the "USB 2-only cable or
port" verdict. This spec does not emit that verdict itself; the combination is the
consumer's (or a later `diagnostic.rs` extension's) call so the two specs stay
independent.

`port.connect_type` was already mentioned to the user as unread; it is listed here
only so the key name is fixed once. `physical_location/*` is already surfaced
elsewhere and is deliberately not redefined here.

## B. Power source and budget (FURTHER-WINS #2)

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `power.source` | `bus` \| `self` (config `bmAttributes` bit 6) | `bmAttributes` readable | `bus` on `usb:5-2.4`, `self` on `usb:3-6` |
| `power.remote_wakeup_capable` | `true` (config `bmAttributes` bit 5) | bit set | `true` on `usb:5-2.4.1` |
| `hub.power_budget_ma` | budget the hub can offer downstream: `500` for a bus-powered USB 2 hub (`900` for USB 3), omitted for self-powered hubs | hub with `power.source == bus` | `500` on `usb:5-2.4` |
| `hub.power_committed_ma` | sum of direct children's `bMaxPower` (children that are themselves self-powered count `0`) | hub with ≥1 child | `188` on `usb:5-2.4` (90 + 98) |

`usb_max_power_ma` (existing key) remains the per-device declared maximum;
unchanged. A future `Bottleneck` variant for over-budget hubs would ride the
existing `charging_diag` tuple and enum-extensibility rule; it is NOT specified here
because no live case exists to validate copy against.

## C. Runtime power-management state (FURTHER-WINS #4)

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `pm.status` | `active` \| `suspended` \| `suspending` \| `resuming` \| `error` \| `unsupported` (kernel `runtime_status` verbatim) | readable | `suspended` on `usb:1-1` |
| `pm.autosuspend` | `auto` \| `on` (kernel `power/control` verbatim; `on` = autosuspend forbidden) | readable | `on` on `usb:5-2.1.1` |
| `pm.wakeup` | `enabled` \| `disabled` (kernel `power/wakeup`; the *policy* — whether the device may wake the system) | file exists (absent on devices with no wakeup capability) | `enabled` on `usb:5-2.4.1`, `disabled` on `usb:5-2.4.2` |
| `connected_s` | `power/connected_duration` / 1000 | readable | `766` on `usb:5-2.1.1` |

**Fingerprint rule (normative for the daemon):** `pm.status`, `pm.autosuspend` and
`connected_s` MUST NOT be included in `state_fingerprint()` (`sysfs/manager.rs`);
otherwise `DeviceChanged` fires on every HID idle transition and every 500 ms tick.
`pm.wakeup` and everything in sections A, B, D MAY be fingerprinted (they are
static for the life of the connection). `port.peer_state` MAY be fingerprinted — a
companion port training late is exactly the kind of transition a consumer wants.

## D. Kernel quirks and authorization (FURTHER-WINS #5)

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `kernel.quirks` | comma-separated quirk names without the `USB_QUIRK_` prefix, in ascending bit order; unknown bits rendered as `bit<N>` | `quirks` != `0x0` | `NO_LPM` on `usb:5-2.1.1` |
| `kernel.avoid_reset` | `true` | `avoid_reset_quirk == 1` | (never on this box) |
| `authorized` | `false` | device `authorized == 0` (omit when authorized) | (never on this box) |

Consumers should render `kernel.quirks` as "Kernel workaround: …" in the technical
details section, and `authorized=false` as a prominent "blocked by USB
authorization policy" status line. Name table source: `include/linux/usb/quirks.h`
of the running kernel; the daemon ships a snapshot (bits 0–18 as of 7.0).

## E. Functional identity (FURTHER-WINS #6)

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `function.net` | network interface name (first `net/*` under any interface) | present | `enxd0c24e46d390` on `usb:5-2.1.1` |
| `function.net.operstate` | `up` \| `down` \| `unknown` … (kernel verbatim) | `function.net` present | `down` |
| `function.net.link_mbps` | `/sys/class/net/<if>/speed` when > 0 | link up and speed readable | (`1000` when cabled) |
| `function.bluetooth` | `hciN` | `bluetooth/hci*` present | `hci0` on `usb:1-1` |
| `function.input` | `;`-joined `input/*/name` values, deduplicated, in inputN order | any `input/input*` present | `Dell Dell USB Keyboard Hub;Dell Dell USB Keyboard Hub Consumer Control;Dell Dell USB Keyboard Hub System Control` |
| `function.block` | `;`-joined block device names | `block/*` present | (none on this box) |
| `function.sound` | `cardN` | `sound/card*` present | (none on this box) |
| `function.video` | `;`-joined `video4linux/video*` names | present | (none on this box) |

`function.net.operstate` and `function.net.link_mbps` are live-ish and MUST NOT be
fingerprinted; `function.net` / `function.bluetooth` / `function.input` MAY be.

## F. Interface strings and configurations (FURTHER-WINS #7, optional tier)

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `interface.<N>.name` | iInterface string from `<dev>:<cfg>.<N>/interface` | non-empty | `interface.0.name = Billboard Interface`, `interface.1.name = Control Interface` on `usb:5-2.1.2` |
| `usb.configurations` | `bNumConfigurations` when > 1 | > 1 | `2` on `usb:5-2.1.1` |

The parsed-descriptor detail (what config #2 *is*, alt-setting ladders) is
intentionally NOT put on the wire; it belongs in `SnapshotJson` (already
serde-`default`-tolerant, additive) for the technical-details view.

## G. Vendor/product database names (FURTHER-WINS #3)

No new key is strictly required: the existing positional `vendor` (pos 9) already
allows "descriptor + vendor-DB fallback", and `headline` (pos 6) is free-form prose.
To keep raw descriptor strings distinguishable from DB names for consumers that
care, one additive key:

| Key | Value | Present when | Example (live) |
|---|---|---|---|
| `product_db` | hwdb / usb.ids model name | lookup hit and differs from `product` | `AX200 Bluetooth` on `usb:1-4`, `RTS5411 Hub` on `usb:5-2` |

Consumers MAY prefer `product_db` over an empty `product` for display.

## H. New signal (FURTHER-WINS #1, over-current deepening)

| Member | Signature | Semantics |
|---|---|---|
| `PortOverCurrent` (signal) | `(ssu)` | `(id, port_id, count)` — `id` is the `usb:<bus_port>` of the **hub** owning the port (or the root-hub controller entry if one is ever exposed; today root hubs are excluded from summaries, so for root-hub ports `id` is the empty string and `port_id` carries the identity), `port_id` is e.g. `5-2.1-port3`, `count` is the new `over_current_count`. Emitted when the daemon's existing libudev monitor sees a `usb` uevent carrying `OVER_CURRENT_PORT` / `OVER_CURRENT_COUNT` (kernel ≥ 4.17), or when a refresh observes the counter increase. Additive; does not bump the interface. |

Rationale for a dedicated signal rather than a property: over-current is an event
(the counter only ever grows), consumers want a toast, and it typically coincides
with the offending device *disappearing*, so there may be no entry left to hang a
property on.

## Not expressible additively — none

Every finding maps onto `properties` or a new signal. The one design that would
have needed a tuple change — a per-device `power.source` field in the `(uuus)`
power tuple — is deliberately expressed as a property instead (section B). If a
future revision wants `port.*` as a typed struct rather than strings, that is a
`Devices6` decision and is not proposed here.

## Coordination checklist with the BOS spec

- No key in this document starts with `bos.` or `link.`; if the BOS spec chooses
  `link.capable_mbps`-style names, section A's `port.peer_state` is the intended
  companion and should be referenced from their consumer guidance, not redefined.
- FURTHER-WINS #5 `kernel.quirks` containing `NO_BOS` is a signal for *their*
  reader to report "unknown" rather than "not SuperSpeed-capable".
- FURTHER-WINS #8 (Billboard) is handed to them wholesale; no key here.
