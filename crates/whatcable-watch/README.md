# whatcable-watch

libudev hotplug monitor for [WhatCable](https://github.com/Zetaphor/whatcable-linux).

A thin wrapper around the [`udev`](https://crates.io/crates/udev) crate
that watches the `usb` and `typec` subsystems and notifies the caller
when devices come or go.

## Two API levels

### Low-level: `Watcher`

For callers integrating udev events into an existing event loop:

```rust
use std::time::Duration;
use whatcable_watch::{Watcher, WaitResult};

let mut w = Watcher::new()?;
match w.wait(Some(Duration::from_secs(60))) {
    WaitResult::Readable => { w.drain(); /* re-enumerate */ }
    WaitResult::Timeout => {}
    WaitResult::Interrupted => {}
}
# Ok::<(), std::io::Error>(())
```

`Watcher::fd()` exposes the raw file descriptor for `epoll(7)` or
`tokio::PollEvented`.

### High-level: `run_loop`

For CLI tools that just want to re-render on every change:

```rust,no_run
use std::time::Duration;
use whatcable_watch::run_loop;

run_loop(Duration::from_millis(500), |_reason| {
    // re-enumerate + render here
    Ok(())
})?;
# Ok::<(), std::io::Error>(())
```

Hooks `SIGINT` / `SIGTERM` for clean shutdown.

## Build

Requires `libudev` development headers on the build host:

```bash
sudo apt install libudev-dev pkg-config       # Debian / Ubuntu
sudo dnf install systemd-devel pkgconf        # Fedora
sudo pacman -S systemd-libs pkgconf           # Arch
```

## License

[MIT](../../LICENSE)
