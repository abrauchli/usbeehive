# Roadmap

Future enhancements under consideration. Entries here are specced but
**not committed** — each needs a go/no-go decision before implementation.
Decided work is tracked in `.planning/` and shipped work in `CHANGELOG.md`.

## Undecided

### Link-speed memory (per-device best-seen speed)

**Status:** undecided · specced 2026-06-10

**Motivation.** A USB 3.x device on a USB 2.0-only cable re-presents
itself with USB 2.x descriptors — a single sysfs snapshot contains
nothing to flag. Verified live with a Pixel 7 (10 Gbps over C-to-C,
480 Mbps over two different A-to-C cables, identical descriptors on
both): the only evidence of degradation exists *across time*. The
daemon is long-running and had seen the same serial link at 10 Gbps
minutes earlier; it just doesn't remember. This is the strongest
"wrong cable/port" signal the app could produce — squarely the
project's headline purpose.

**Design sketch.**

- In-memory map in the daemon (likely `SysfsManager`):
  `VID:PID + serial → best link speed seen (Mbps)`. Track only
  devices with a non-empty serial to avoid collisions. No
  persistence in v1 — a live cable swap is the headline use case;
  across-reboot memory (XDG state dir) is a possible v2.
- On enumeration below the best-seen speed, emit an additive
  property on the USB device entry:
  `usb.link_speed_drop = "<best-seen speed label>"` (e.g.
  `"10 Gbps"`).
- CLI: yellow hint line, e.g.
  `Linked at 480 Mbps — this device previously reached 10 Gbps`.
- Wording must stay a neutral hint, never "bad cable": a drop is
  equally caused by a 2.0-only hub/port on the host side.

**Wire impact.** None — additive `properties` key under the
documented org.usbeehive.Devices5 conventions.

**usbee impact.** Label-table entry for `usb.link_speed_drop`
(+ pot entry, CHANGELOG). Later candidate for the existing
notification machinery (port-degradation notifier), but v1 is a
popover row only.

**Open questions.**

- Eviction: unbounded map is fine for a laptop's device population,
  but should hot-unplug clear entries, or is "best ever this
  daemon-lifetime" the right semantic? (Leaning: keep for daemon
  lifetime; that's what makes the cable-swap case work.)
- Should the CLI one-shot mode (`--list`) share the memory? It runs
  a fresh process per invocation, so it would only benefit if the
  daemon is queried over D-Bus instead of reading sysfs directly.
- Speed regression vs. deliberate slow link (user plugs into a 2.0
  hub on purpose): acceptable as a hint, or does it need a per-device
  mute like port notifications have in usbee?

**Rejected alternatives** (from the 2026-06-10 cable experiment):

- Detecting capability from descriptors on a 2.0 link — impossible;
  the device legitimately re-presents itself as USB 2.x.
- Grading A-to-C cables beyond data wiring — BC 1.2 / charging
  current quality is invisible in sysfs; 2.0-only A-to-C cables of
  different build quality are protocol-indistinguishable.
