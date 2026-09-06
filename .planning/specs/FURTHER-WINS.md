# FURTHER-WINS — unexploited kernel primitives for usbeehive 0.11.0

> **Status:** survey document. Findings #1 (minus over-current), #2 (minus
> remote wakeup), #3 and #5 (`quirks` only) shipped in quick task
> 260905-tb1; #4, #6, #7, #9, #10 are deferred. See the status table at the
> top of `DBUS-FURTHER-SPEC.md`, and `DBUS-TRIM-CONSUMER-SPEC.md` for what
> the daemon actually emits today.

Probed on: Ubuntu 26.04.1, kernel 7.0.0-31-generic, AMD desktop (3x xHCI 1022:149c,
6 root hubs, no Type-C/PD/Thunderbolt class devices). All sample values below were
read live from this machine on 2026-09-05. Repos were not modified.

Out of scope by instruction (owned elsewhere / already surfaced): `bos_descriptors`
link-speed work, `physical_location/`, and the bare existence of port
`connect_type` / `over_current_count` / `state`. Where those are touched below it is
only to add something material on top.

## Novelty verdict, up front

Nothing found here is new in kernel 7.0. The kernel's own ABI documentation
(`Documentation/ABI/testing/sysfs-bus-usb`, fetched from torvalds/master, 607 lines)
has **no entry newer than March 2024** (`bos_descriptors`). Approximate age of every
primitive used below, mapped from the ABI doc `Date:` field (mapping to kernel
numbers is my approximation, not authoritative):

| Primitive | ABI doc Date | Approx. kernel | Verdict |
|---|---|---|---|
| device `authorized` | Jul 2008 | 2.6.26 (documented KernelVersion) | long-available |
| `power/wakeup`, `power/control` | Jan 2009 | 2.6.3x | long-available |
| `power/runtime_status`, `autosuspend_delay_ms`, `wakeup_*` | Apr/Sep 2010 | 2.6.35–36 | long-available |
| `bmAttributes`, `bcdDevice`, `descriptors`, `urbnum`, `active_duration`, `connected_duration`, `ep_*` | undated | pre-3.0 | long-available |
| `ltm_capable` | Jul 2012 | 3.6 | long-available |
| port `connect_type` | Jan 2013 | 3.8 | long-available |
| port `peer` symlink | not in ABI doc | ~3.17 (2014) — **unverified** | long-available |
| interface `authorized`, `interface_authorized_default` | Aug 2015 | 4.4 | long-available |
| port `usb3_lpm_permit` | Nov 2015 | 4.4 | long-available |
| port `over_current_count` (+ udev event, poll()) | Feb 2018 | 4.17 | long-available |
| port `location` | Oct 2018 | 4.20 | long-available |
| port `connector` → Type-C | Dec 2021 | 5.17 | long-available, untestable here |
| `physical_location/` | Mar 2022 | 5.19 | (already known) |
| port `disable` | Jun 2022 | 6.0 | long-available |
| port `early_stop` | Sep 2022 | 6.2 | long-available |
| port `state` (poll()-able) | Jun 2023 | 6.5 | recent-ish, not 7.0 |
| device `typec` symlink → partner | Nov 2023 | 6.8 | recent-ish, untestable here |
| `bos_descriptors` | Mar 2024 | 6.10 | (owned by other agent) |
| `quirks` bit 17 `USB_QUIRK_NO_BOS`, bit 18 `FORCE_ONE_CONFIG` | from `/usr/src/linux-headers-7.0.0-31/include/linux/usb/quirks.h` | 6.x | recent-ish |

So: "what new USB primitives arrived with 7.0" has the honest answer **none that
this crate needs**. The value is in long-available things the crate never read.

## What the crate reads today (baseline for the diff)

`src/sysfs/usb.rs` consumes exactly: `idVendor idProduct bDeviceClass bDeviceSubClass
bDeviceProtocol manufacturer product serial version removable speed bMaxPower busnum
devnum rx_lanes tx_lanes bNumConfigurations bNumInterfaces`, per-interface
`bInterfaceClass bInterfaceSubClass bInterfaceProtocol driver`, plus a blind
`read_all_attrs` dump into `raw_attributes` (only surfaced by `--raw`, never
interpreted). It never descends into `power/`, `ep_*`, `*-port*`, `port`, `peer`,
or into interface children (`net/`, `bluetooth/`, `input/`).

Live top-level attribute union across the 38 sysfs nodes on this box (count = nodes
carrying it): `authorized`(38) `power/`(38) `driver`(37) `firmware_node`(24)
`supports_autosuspend`(21) `bNumEndpoints`(21) `bAlternateSetting`(21) `urbnum`(17)
`quirks`(17) `maxchild`(17) `ltm_capable`(17) `ep_00`(17) `descriptors`(17)
`configuration`(17) `bmAttributes`(17) `bcdDevice`(17) `bMaxPacketSize0`(17)
`bConfigurationValue`(17) `avoid_reset_quirk`(17) `devpath`(17) `physical_location`(12)
`port`(11) `interface`(7) `bos_descriptors`(7) `authorized_default`(6)
`interface_authorized_default`(6) `wakeup`(2) `coredump`(4), plus 41 `*-portN` dirs.

---

## Ranked findings

### 1. Physical-connector model: `port` → `peer` → companion `state` (USB2/USB3 lane pairing)

**What.** Every non-root device carries a `port` symlink to the hub-port object it
hangs off. Every root-hub port carries a `peer` symlink to the port on the *other*
root hub of the same xHCI that shares the same physical connector (USB 2.0 half ↔
USB 3.x half). The companion port's `state` tells you whether the SuperSpeed lanes
of that connector ever trained.

**Paths.**
`/sys/bus/usb/devices/<dev>/port` → `../../usbX-portY`
`/sys/bus/usb/devices/usbX/usbX-portY/peer` → `../../usbZ-portY`
`/sys/bus/usb/devices/usbX/usbX-portY/state`, `.../location`, `.../maxchild` on the hub.

**Live sample (this machine).**
```
5-2/port      -> usb5-port2          # RTS5411 hub, enumerated at 480 Mb/s
usb5-port2/peer -> usb6-port2        # usb6 = 10 Gb/s root hub of the same xHCI
usb6-port2/state = not attached      # SS lanes of that connector saw nothing
usb5-port2/location = 0x80000002 == usb6-port2/location   (same connector)
5-2: bDeviceProtocol=02 version=2.10 bos_descriptors=42 bytes (SS capability present)
```
Full peer table: usb1↔usb2 (ports 1,3), usb3↔usb4 (1–4), usb5↔usb6 (1–4). Every
device→port link: `1-1→usb1-port1 1-4→usb1-port4 3-5→usb3-port5 3-6→usb3-port6
5-2→usb5-port2 5-2.1→5-2-port1 5-2.4→5-2-port4 5-2.1.1→5-2.1-port1 …`.

**Why it serves the mission.** This is the "what can this cable actually do" question
answered from the *port* side, complementary to the BOS agent's *device* side. The
BOS work can say "device 5-2 claims SuperSpeed but negotiated 480". This adds
*where and why*: the connector's SS companion port is `not attached` → the SS wires
never linked → USB-2-only cable, or a USB-A 2.0 receptacle. If the companion were
`powered`/`reconnecting` instead, that points at a flaky SS link rather than a
missing one. It also enables collapsing the two logical root-hub ports into one
"connector" entry in the extension, and hub occupancy ("4-port hub, 2 in use") via
`maxchild` + child port `state`s. Note `location` is only unique per controller
(usb1-port1 and usb5-port2 both read `0x80000002`); `peer` is the authoritative
join, `location` is a fallback.

**Novelty.** Long-available (`peer` ~2014; `state` 2023 / ~6.5). Nothing 7.0-specific.

**Cost/risk.** Low. Two `read_link` + one `read_attr` per device, plus a
`maxchild`/port-state loop per hub. Pure additive fields on `UsbDevice`. No fixtures
in the repo have `*-portN` dirs today — fixtures would need extending. Root hubs
are excluded from summaries, so the "connector" grouping is a summary-layer change.

**Untestable here.** Type-C-attached connectors (where `port/connector` also
exists, see #9); SS companion states other than `not attached`/`configured`/`suspended`.

**Material deepening of the already-mentioned port attrs.** `over_current_count`
and `state` both support `poll()` and the hub emits a udev event
`OVER_CURRENT_PORT=…`/`OVER_CURRENT_COUNT=…` on change (ABI doc text). The daemon
already holds a libudev monitor (`src/watch.rs`), so an over-current event can be
turned into a D-Bus signal for free, without the 500 ms refresh loop noticing it.

### 2. Bus-/self-powered flag and hub power budget (`bmAttributes` + `bMaxPower` + topology)

**What.** Config descriptor `bmAttributes` bit 6 = self-powered, bit 5 = remote
wakeup. The crate already reads `bMaxPower` but not `bmAttributes`, so it cannot
tell a device that *draws* 500 mA from one that merely *declares* it while
self-powered, and it cannot compute what a bus-powered hub's children add up to.

**Paths.** `/sys/bus/usb/devices/<dev>/bmAttributes` (hex), `bMaxPower`, `maxchild`,
plus the existing parent/child topology.

**Live sample.**
```
3-6     bmAttributes=e0 (self-powered, remote-wakeup) bMaxPower=100mA  hub, 4 ports
5-2     bmAttributes=e0 (self-powered)                bMaxPower=0mA    hub, 4 ports
5-2.4   bmAttributes=a0 (BUS-powered, remote-wakeup) bMaxPower=100mA  hub, 3 ports, 12 Mb/s
  5-2.4.1 a0 bus-powered 90mA  (keyboard)
  5-2.4.2 a0 bus-powered 98mA  (mouse)
5-2.1.1 bmAttributes=a0 (bus-powered) bMaxPower=350mA  RTL8153 GbE
5-2.1.2 bmAttributes=c0 (self-powered, no wakeup) 100mA  Samsung monitor billboard
3-5     bmAttributes=80 (bus-powered, no wakeup) 500mA  MSI Mystic Light
1-1     bmAttributes=e0 (self-powered) 500mA           TP-Link BT (declares 500 yet self-powered)
```

**Why it serves the mission.** "Why did my drive drop out when I plugged in the
webcam" is a bus-power-budget question. A bus-powered hub has 500 mA (USB 2) total
and per the spec 100 mA per downstream port; the crate can sum `bMaxPower` of its
children and warn when the committed draw exceeds the budget, or when a 350 mA
device sits behind a bus-powered hub. Also drives a correct "Powered by: bus / own
supply" line and "Can wake the PC" flag per device. Cheap, and it is exactly the
kind of actionable verdict `diagnostic.rs` already produces for PD.

**Novelty.** Long-available (undated in ABI doc, present since 2.6).

**Cost/risk.** Very low for the flags (one hex read). Low-medium for the budget
diagnostic (needs a hub-level pass in `summary.rs`; must not double-count when a
child is itself a self-powered hub). Heuristic caveat: `bMaxPower` is a declared
maximum, not measured draw — word it "declares", never "draws".

**Untestable here.** Over-budget case (nothing on this box exceeds it); USB 3 900 mA
budgets (no SS device attached).

### 3. Vendor/product names from udev hwdb (`ID_VENDOR_FROM_DATABASE`, `ID_MODEL_FROM_DATABASE`) or `/usr/share/misc/usb.ids`

**What.** The crate's `vendor.rs` is a ~4-entry match table and there is no product
lookup at all. libudev — already a dependency via the `watch` feature — exposes the
hwdb-resolved names for every USB device with zero bundled data.

**Paths.** `udev::Device::from_syspath(...).property_value("ID_MODEL_FROM_DATABASE")`;
fallback file `/usr/share/misc/usb.ids` (730 KB, present); hwdb at
`/usr/lib/udev/hwdb.bin` (14 MB, present).

**Live sample.** Current `usbeehive --tree` prints `8087:0029` as the headline of
device 1-4 (empty `product`); hwdb says `Intel Corp.` / `AX200 Bluetooth`. Hub 5-2
is headlined by its iProduct `4-Port USB 2.0 Hub`; hwdb says `RTS5411 Hub`, which
is a USB 3.0 hub — a user-visible clue that ties directly into finding #1.
`5-2.1.1` → `RTL8153 Gigabit Ethernet Adapter`. `usb.ids` has all of
`8087:0029 0bda:5411 0bda:8153 05e3:0608`.

**Why it serves the mission.** A device you cannot name is a device you cannot
reason about; and the DB name frequently encodes capability the iProduct string
hides (RTS5411 = USB 3.0 hub; RTL8153 = Gigabit).

**Novelty.** Long-available. `UI_PLAN.md` row 77 already lists "Vendor DB: PARTIAL
— port the upstream USB-IF TSV". This finding is the cheaper route: no bundled
table, product names included, and it stays current with the distro.

**Cost/risk.** Very low behind the `watch`/`udev` feature; a plain `usb.ids` parser
for the non-udev build is ~60 lines. Risk: hwdb names can be stale or wrong for
re-badged PIDs — treat as fallback when `product` is empty, or as a secondary
`product_db` property, never as an override.

### 4. Runtime-PM / activity state: `power/runtime_status`, `control`, `wakeup`, `connected_duration`, `active_duration`

**What.** Per-device power-management state and lifetime counters. Also the
hub-port `state=suspended` mirror.

**Paths.** `/sys/bus/usb/devices/<dev>/power/{runtime_status,control,runtime_enabled,
autosuspend_delay_ms,wakeup,connected_duration,active_duration}`; `urbnum` at the
device root.

**Live sample.**
```
dev      control status    enabled    wakeup   connected  active   autosusp
1-1      auto    suspended enabled    disabled 13335 s    34 s     2000 ms   (TP-Link BT)
1-4      auto    suspended enabled    disabled 13318 s    26 s     2000 ms
3-5      on      active    forbidden  (absent) 13335 s    13335 s  2000 ms   (Mystic Light)
5-2.1.1  on      active    forbidden  enabled  766 s      766 s    2000 ms   (RTL8153, replugged 12 min ago)
5-2.4.1  on      active    forbidden  enabled  13334 s    13334 s  2000 ms   (keyboard; can wake PC)
5-2.4.2  on      active    forbidden  disabled 13333 s    13333 s  2000 ms   (mouse; cannot wake PC)
usb1-port1/state = suspended   (mirrors 1-1)
```

**Why it serves the mission.** Three user-facing facts fall out: (a) "asleep vs
awake" — `runtime_status`; (b) "can wake the computer" — `power/wakeup` (the
keyboard yes, the mouse no — a common support question); (c) "connected since" —
`connected_duration` gives an honest connect timestamp even for devices that were
present before the daemon started, which the daemon's own hot-plug log cannot. The
`control=on` + `runtime_enabled=forbidden` pattern explains "why doesn't this device
ever power-save" (udev rule / driver forbids it).

**Novelty.** Long-available (2009–2010).

**Cost/risk.** Very low to read. **Risk in the diff engine:** `runtime_status` flips
often for HID/BT; it must be excluded from `state_fingerprint()` in
`sysfs/manager.rs` or `DeviceChanged` will chatter. Expose it, but don't fingerprint it
(or fingerprint only `wakeup`, which is static).

### 5. Kernel quirk table and authorization: `quirks`, `avoid_reset_quirk`, `authorized`, `authorized_default`

**What.** `quirks` is the bitmask the kernel applied to this device from its
built-in quirk table (or `usbcore.quirks=`). `authorized=0` means the device is
present but blocked by USB authorization policy (USBGuard-style);
`authorized_default` (root hubs) = 0/1/2 shows the policy (2 = internal-only).

**Paths.** `/sys/bus/usb/devices/<dev>/quirks` (hex), `avoid_reset_quirk`,
`authorized`; `/sys/bus/usb/devices/usbX/{authorized_default,interface_authorized_default}`;
per-interface `authorized`. Bit meanings: `/usr/src/linux-headers-7.0.0-31/include/linux/usb/quirks.h`.

**Live sample.** `5-2.1.1/quirks = 0x400` → bit 10 = `USB_QUIRK_NO_LPM` ("link
power management disabled for this device by the kernel quirk table"). All others
`0x0`. `authorized=1` everywhere; `authorized_default=1`,
`interface_authorized_default=1` on all six root hubs. `usbcore.quirks=` empty.

Full bit table from the 7.0.0-31 headers: 0 STRING_FETCH_255, 1 RESET_RESUME,
2 NO_SET_INTF, 3 CONFIG_INTF_STRINGS, 4 RESET, 5 HONOR_BNUMINTERFACES, 6 DELAY_INIT,
7 LINEAR_UFRAME_INTR_BINTERVAL, 8 DEVICE_QUALIFIER, 9 IGNORE_REMOTE_WAKEUP, 10 NO_LPM,
11 LINEAR_FRAME_INTR_BINTERVAL, 12 DISCONNECT_SUSPEND, 13 DELAY_CTRL_MSG,
14 HUB_SLOW_RESET, 15 ENDPOINT_IGNORE, 16 SHORT_SET_ADDRESS_REQ_TIMEOUT,
17 NO_BOS, 18 FORCE_ONE_CONFIG.

**Why it serves the mission.** "The kernel knows this device is buggy and is
working around it" is diagnostic gold when a user asks why a device misbehaves or
never suspends; `RESET_RESUME` explains devices that re-enumerate after every
sleep. `authorized=0` explains "present in lsusb but dead". **Cross-reference for
the BOS agent:** bit 17 `NO_BOS` means `bos_descriptors` is deliberately absent —
their reader should treat that as "unknown", not "no SuperSpeed capability".

**Novelty.** Long-available (`authorized` 2.6.26; `quirks` attr ~4.x; bits 17–18 6.x).

**Cost/risk.** Very low; a bit-to-name table that should be regenerated from the
running kernel's headers when they move.

### 6. Functional children: `net/`, `bluetooth/`, `input/`, `hidraw/`, `block/`, `sound/`, `video4linux/` under the interface dir

**What.** What the device *is* to the OS, and the functional link rate behind the
USB link rate.

**Paths.** `/sys/bus/usb/devices/<dev>/<dev>:<if>/{net,bluetooth,usbmisc}/*`,
`/<dev>:<if>/<hid-id>/{input,hidraw}/*`, and for storage/audio/video
`…/host*/target*/*/block/*`, `…/sound/card*`, `…/video4linux/video*`. Walk depth ≤ 5,
follow the device symlink (`find -H`) but not the `driver`/`subsystem` links.

**Live sample.**
```
5-2.1.1:1.0/net/enxd0c24e46d390      operstate=down carrier=0 speed=-1 (no cable; would be 1000 when up)
1-1:1.0/bluetooth/hci0 (+rfkill0)     1-4:1.0/bluetooth/hci1 (+rfkill3)
5-2.4.1:1.0/0003:413C:2006.0003/input/input3 name="Dell Dell USB Keyboard Hub"
5-2.4.1:1.1/…/input/input4 "… Consumer Control", input5 "… System Control"
5-2.4.2:1.0/…/input/input6 "Logitech USB-PS/2 Optical Mouse"
3-5:1.0/…/input/input2 "MSI MYSTIC LIGHT", usbmisc/hiddev0
5-2.1.2:1.1/…/hidraw/hidraw1, usbmisc/hiddev1   (monitor's HID control channel)
```

**Why it serves the mission.** (a) Names like `enx…`/`hci0` let the extension say
"this is your Ethernet adapter, currently down" rather than "Vendor-specific class".
(b) Layered bottleneck: a Gigabit NIC (`net/*/speed=1000` when up) behind a
480 Mb/s USB link is capped at roughly 300 Mb/s effective — the *consequence* of
the SS-lane failure that #1 and the BOS work detect. (c) Storage: `block/sdX` +
`queue/rotational` tells you which USB link a slow disk is behind.

**Novelty.** Long-available.

**Cost/risk.** Medium: a bounded directory walk per interface, and the net `speed`
attr returns `-1`/EINVAL when the link is down (must tolerate). Fixture-heavy.

### 7. Full `descriptors` blob: inactive configurations, inactive alt-settings, multi-TT hubs, `interface` strings

**What.** sysfs shows only the *active* configuration and alt-setting; the raw
`descriptors` file holds every configuration and alt-setting. The `interface`
attribute on each interface dir is the iInterface string, unread today.

**Paths.** `/sys/bus/usb/devices/<dev>/descriptors` (binary; `stat` reports 65553,
actual reads are 59–195 bytes here), `<dev>:<if>/interface`.

**Live sample (parsed with a 40-line parser, `scratchpad/parse_desc.py`).**
```
5-2.1.1 RTL8153: 2 configurations —
   #1 vendor-specific (r8152 driver, active)   #2 CDC-ECM/NCM (class 02/06 + 0a), alt 1 with bulk EPs
5-2 RTS5411 hub: IF0 alt0 proto=01 (single TT), alt1 proto=02 (multi TT)  ← active alt=1 → multi-TT
1-1 TP-Link BT: IF1 alt 0..5 isoc wMaxPacket 0,9,17,25,33,49 (SCO bandwidth ladder)
interface strings: 1-1:1.0 "Bluetooth Radio", 5-2.1.2:1.0 "Billboard Interface",
                   5-2.1.2:1.1 "Control Interface", 5-2.4:1.0 "Dell USB Keyboard Hub"
```

**Why it serves the mission.** "Hub is multi-TT" matters for anyone hanging USB 1.1
devices (this very box: the 12 Mb/s Dell keyboard hub sits behind 5-2) — with a
single-TT hub all full/low-speed devices share one 12 Mb/s transaction translator.
"This adapter also offers a standard CDC-NCM configuration" tells power users the
device works without the vendor driver. The `interface` string is a free, better
subtitle for multi-function devices.

**Novelty.** Long-available.

**Cost/risk.** Medium (binary descriptor walker; the crate already has this kind
of decoder in `pd.rs`/`cable.rs`). Multi-TT alone is cheaper via
`bDeviceProtocol==2` on the hub device (already read but unused). Ranked below #6
because only the multi-TT bit is a common user-facing win.

### 8. Billboard capability descriptor (in BOS) — CROSS-REFERENCE ONLY, owned by the BOS agent

**What.** USB Billboard Device Class (interface class 0x11, which `usbclass.rs`
already names) exists precisely to report *which Alternate Mode was attempted and
why it failed* — the core of "why is my monitor's USB-C not doing DisplayPort".
The Billboard Capability Descriptor (bDevCapabilityType 0x0D) lives inside
`bos_descriptors`, which the other agent owns. Do not implement independently;
hand them this decode.

**Live sample.** `5-2.1.2/bos_descriptors` (88 bytes) decoded: capability 0x0D,
bcdVersion 1.21, `bNumberOfAlternateOrUSB4Modes=1`, AUM[0] `wSVID=0xFF01`
(DisplayPort), `bmConfigured[0]=01b` = "Alternate Mode configuration not attempted
or exited", `bAdditionalFailureInfo=0x02` = "no USB-PD communication", followed by
a Billboard AUM Capability descriptor (type 0x0F) with `dwAlternateModeVdo=0x001c0045`.
Plain English: the Samsung monitor is connected through a path with no PD channel
(USB-A → C cable on a desktop) so DP-alt-mode was never attempted. The USB link is
12 Mb/s HID/billboard only.

**Limitation.** `iAdditionalInfoURL`/`iAlternateOrUSB4ModeString` are string
descriptors; sysfs exposes only manufacturer/product/serial/configuration/interface
strings, so the vendor's explanatory URL cannot be read passively.

**Untestable here.** Any Billboard state other than 01b; USB4 mode billboards.

### 9. Kernel-canonical USB↔Type-C linkage: `port<X>/connector` and device `typec` symlinks

**What.** `manager.rs` pairs a Type-C port to its USB device via the partner's USB
child directory name (heuristic, comment says "the kernel's canonical linkage" —
it is not). The kernel provides two explicit links: hub-port → Type-C connector
(`connector`, Dec 2021, ~5.17) and USB device → Type-C partner (`typec`, Nov 2023,
~6.8). On 6.8+ laptops this makes the pairing exact and removes the single-pd +
single-port fallback ambiguity.

**Live sample.** None possible — no `/sys/class/typec` on this box. Verified only via
the ABI document text. Zero-cost to *attempt* (read_link, tolerate ENOENT).

**Novelty.** `typec` symlink is the most recent primitive in this report (~6.8) —
still predates 7.0.

### 10. xHCI upstream PCIe link (`../current_link_speed`, `max_link_speed`, `*_link_width`)

**What.** The root hub's parent is the PCI xHCI function; its PCIe link caps
aggregate USB bandwidth (relevant for add-in USB 3.2 Gen 2×2 cards on x1 slots or
chipset lanes in a degraded state).

**Live sample.** All three controllers: `1022:149c`, `16.0 GT/s PCIe x16` current
== max; no bottleneck here. Root-hub `serial` already carries the PCI address
(`0000:2a:00.1`), so the join is trivial.

**Verdict.** Real but niche; include only as a `--raw`/technical-details line.

---

## Considered and rejected

| Candidate | Live value | Why rejected |
|---|---|---|
| `ltm_capable` | `yes` on SS root hubs, `no` elsewhere | USB 3 Latency Tolerance Messaging is invisible to users; no action follows. |
| `bcdDevice` | e.g. `3100`, `4800`, `0150` | Firmware revision is trivia unless paired with a known-bad-firmware DB the crate doesn't have. Fine for `--raw`, which already shows it. |
| `bMaxPacketSize0`, `devpath`, `bConfigurationValue`, `bNumEndpoints`, `bAlternateSetting` | various | Already in `raw_attributes`; no user-facing meaning on their own. |
| `configuration` (iConfiguration string) | empty on 16/17 devices; `83NT6504V110` on the Dell keyboard | Almost always empty; when present it's a part number. |
| `ep_*` sysfs dirs | `type=Bulk/Interrupt/Isoc`, `interval=1ms…255ms` | Only the *active* alt-setting; #7's `descriptors` blob supersedes it. Endpoint polling intervals are not user-actionable. |
| `supports_autosuspend` | `1` on all 21 interfaces | Constant here; only meaningful as a "never" flag on rare drivers. Folded conceptually into #4. |
| `power/persist`, `power/async`, `runtime_usage`, `runtime_active_kids` | `1`, `enabled`, … | PM-core internals. |
| `power/wakeup_*count/time` | all `0` | Only interesting after a suspend cycle; a desktop that never sleeps shows nothing. Could be a later "this device woke the PC N times" line; not now. |
| `urbnum` | `17718`, `10774`, `668`, … | URB *count*, not bytes — cannot give bandwidth. Delta between polls is a crude "is anything happening" blink at best; deferred, noted under #4. |
| port `usb3_lpm_permit` | `u1_u2` | Policy knob for LPM-hostile devices; reading it only tells you the default. |
| port `disable` | `0` everywhere | Write knob (Vbus/port off). Reading `1` would be worth a "port administratively disabled" line — fold into #1 if ever non-zero; no independent value. |
| port `early_stop` | `no` | Android-ramdump watchdog knob; irrelevant on desktops. |
| port `quirks` | `00000000` | Enumeration-timing tweak bits; no user meaning. |
| port `location` (raw) | `0x80000002`, `0x80000105` … | Non-unique across controllers; `peer` (#1) and `physical_location/` (already known) express it better. |
| `firmware_node` symlink → ACPI (`\_SB_.PC00.XHCI.RHUB.HS01`-style) | present on 24 nodes | ACPI naming is trivia for users; `physical_location/` is the digested form. |
| `waiting_for_supplier` | present on all 41 ports | Device-links core internal. |
| `coredump` | 4 interfaces (btusb) | Write trigger for driver coredump; not passive. |
| `/sys/class/wakeup/wakeup43,44` | `name=5-2.4.1`, `5-2.1.1`, `event_count=0` | Duplicate of `power/wakeup*` per device. |
| `/sys/class/usb_role` | empty here | Dual-role (OTG) switching; laptop/phone-class hardware only. Untestable; low value even there since `data_role` on the Type-C port already covers it. |
| `/sys/class/power_supply` | empty here | Already handled by the crate on laptops. |
| debugfs (`/sys/kernel/debug/usb`), usbmon | `Permission denied` | Root-only; `usbeehived.service` is a `--user` unit (`WantedBy=default.target`). Not passive-read-only friendly. |
| usbfs ioctls (`/dev/bus/usb/*`) for hub descriptors / port status / string descriptors | n/a | Generates bus traffic and needs device-node access — contradicts the passive read-only design. This is the only way to get Billboard URL strings or hub descriptor bits (per-port power switching, compound device); explicitly not recommended. |
| udev `ID_USB_INTERFACES`, `ID_PATH`, `ID_SERIAL` | `:ffff00:020600:0a0000:` etc. | Redundant with sysfs data the crate already has. (The `*_FROM_DATABASE` props are #3.) |
| `/sys/module/usbcore/parameters/*` | `autosuspend=2 authorized_default=1 quirks=` | Global defaults; only `quirks=` (user-added) has diagnostic value and is already reflected per-device in #5. |
| Multi-configuration switch detection via `bNumConfigurations` alone | `2` on 5-2.1.1 only | Already read; without #7 it cannot say what config #2 *is*. |

## Second deliverable

Findings #1, #2, #4, #5, #6 (and #3 indirectly) all want to reach the GNOME
extension; every one of them fits in the additive `properties` bag plus one new
signal. `DBUS-FURTHER-SPEC.md` in this directory specifies the contract. Nothing
proposed requires touching an existing tuple shape.
