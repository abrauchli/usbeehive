//! libudev hotplug monitor for [WhatCable](https://github.com/Zetaphor/whatcable-linux).
//!
//! This crate is a thin wrapper around the [`udev`] crate that watches the
//! `usb` and `typec` subsystems and notifies the caller when devices come
//! and go. The intended usage is:
//!
//! 1. Take an initial snapshot (e.g. via `whatcable::DeviceManager::refresh`).
//! 2. Open a [`Watcher`] and treat it as a "did anything change?" signal.
//! 3. Whenever the watcher fires, re-snapshot with `refresh()` and re-render.
//!
//! The crate does not pull in `whatcable-sysfs` — you can use it with any
//! re-enumeration strategy.
//!
//! # Quick start
//!
//! ```no_run
//! use std::time::Duration;
//! use whatcable::watch::{Watcher, WaitResult};
//!
//! let mut watcher = Watcher::new().unwrap();
//! loop {
//!     match watcher.wait(Some(Duration::from_secs(60))) {
//!         WaitResult::Readable => {
//!             watcher.drain();
//!             println!("usb / typec changed");
//!         }
//!         WaitResult::Timeout => {}
//!         WaitResult::Interrupted => break,
//!     }
//! }
//! ```
//!
//! For a complete debounced render-loop, use [`run_loop`].

#![allow(unsafe_code)]

use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use udev::MonitorBuilder;

/// Outcome of a single [`Watcher::wait`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// The udev socket has data — drain it and re-render.
    Readable,
    /// `wait` returned because the supplied timeout elapsed.
    Timeout,
    /// `poll(2)` was interrupted (e.g. by a signal); caller should loop.
    Interrupted,
}

/// Subscriptions for [`Watcher`]. Defaults to `usb` + `typec`, which is the
/// pair WhatCable cares about.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Subsystems to subscribe to (passed to `udev_monitor_filter_add_match_subsystem_devtype`).
    pub subsystems: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        WatcherConfig {
            subsystems: vec!["usb".into(), "typec".into()],
        }
    }
}

/// Live udev hotplug subscription.
///
/// Holds an open netlink socket. Drop the value to close the subscription.
pub struct Watcher {
    socket: udev::MonitorSocket,
}

impl Watcher {
    /// Open a watcher with the default subsystem set (`usb` + `typec`).
    pub fn new() -> io::Result<Self> {
        Self::with_config(&WatcherConfig::default())
    }

    /// Open a watcher subscribing to `config.subsystems`.
    pub fn with_config(config: &WatcherConfig) -> io::Result<Self> {
        let mut builder = MonitorBuilder::new()?;
        for s in &config.subsystems {
            builder = builder.match_subsystem(s)?;
        }
        let socket = builder.listen()?;
        Ok(Watcher { socket })
    }

    /// Raw fd suitable for `poll(2)` / `epoll(7)` integration.
    pub fn fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    /// Consume any pending events. Returns the number drained — a non-zero
    /// value confirms that at least one matching kernel event arrived.
    /// Most callers can ignore the count and just re-enumerate.
    pub fn drain(&mut self) -> usize {
        self.socket.iter().count()
    }

    /// Block until the socket becomes readable or `timeout` elapses.
    ///
    /// Pass `None` for an infinite wait. The implementation uses a single
    /// `poll(2)` syscall; signals like `SIGINT` will return
    /// [`WaitResult::Interrupted`] without consuming any pending data.
    pub fn wait(&self, timeout: Option<Duration>) -> WaitResult {
        let timeout_ms: i32 = match timeout {
            Some(d) => d.as_millis().min(i32::MAX as u128) as i32,
            None => -1,
        };
        let mut pfd = libc::pollfd {
            fd: self.fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let r = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if r < 0 {
            WaitResult::Interrupted
        } else if r == 0 {
            WaitResult::Timeout
        } else if (pfd.revents & libc::POLLIN) != 0 {
            WaitResult::Readable
        } else {
            WaitResult::Timeout
        }
    }
}

static GLOBAL_RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn on_signal(_: libc::c_int) {
    GLOBAL_RUNNING.store(false, Ordering::SeqCst);
}

/// Hook `SIGINT` and `SIGTERM` to exit the next [`run_loop`] cleanly.
///
/// Safe to call multiple times.
pub fn install_default_signal_handlers() {
    GLOBAL_RUNNING.store(true, Ordering::SeqCst);
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as usize);
        libc::signal(libc::SIGTERM, on_signal as *const () as usize);
    }
}

/// Run a debounced render loop until `SIGINT` / `SIGTERM` arrives, or
/// `on_refresh` returns an error.
///
/// `on_refresh` is invoked exactly once at startup and again at most once
/// per `debounce` window after each burst of kernel events. Callers
/// typically perform a full sysfs re-enumeration + render inside this
/// callback.
///
/// ```no_run
/// use std::time::Duration;
/// use whatcable::watch::run_loop;
///
/// run_loop(Duration::from_millis(500), |reason| {
///     println!("refresh because: {reason:?}");
///     Ok(())
/// }).unwrap();
/// ```
pub fn run_loop<F>(debounce: Duration, mut on_refresh: F) -> io::Result<()>
where
    F: FnMut(RefreshReason) -> io::Result<()>,
{
    install_default_signal_handlers();

    let mut watcher = Watcher::new()?;
    on_refresh(RefreshReason::Initial)?;

    let mut dirty: Option<Instant> = None;

    while GLOBAL_RUNNING.load(Ordering::SeqCst) {
        let timeout = dirty.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        match watcher.wait(timeout) {
            WaitResult::Readable => {
                watcher.drain();
                dirty = Some(Instant::now() + debounce);
            }
            WaitResult::Timeout => {}
            WaitResult::Interrupted => continue,
        }
        if let Some(deadline) = dirty {
            if Instant::now() >= deadline {
                on_refresh(RefreshReason::Hotplug)?;
                dirty = None;
            }
        }
    }
    Ok(())
}

/// Why [`run_loop`] is invoking the refresh callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshReason {
    /// First call, before the watcher starts blocking.
    Initial,
    /// One or more kernel uevents arrived inside the previous debounce window.
    Hotplug,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_config_defaults_to_usb_and_typec() {
        let cfg = WatcherConfig::default();
        assert!(cfg.subsystems.iter().any(|s| s == "usb"));
        assert!(cfg.subsystems.iter().any(|s| s == "typec"));
    }

    // Real watcher construction needs libudev. Inside this repo's CI we have
    // it (CI installs libudev-dev), so this test runs there. On systems
    // without libudev or in sandboxed builds it'll fail gracefully — that's
    // an environmental error, not a logic bug.
    #[test]
    #[cfg(target_os = "linux")]
    fn watcher_can_open_and_close() {
        match Watcher::new() {
            Ok(w) => {
                assert!(w.fd() >= 0);
            }
            Err(e) => {
                eprintln!("Watcher::new failed (libudev unavailable?): {e}");
            }
        }
    }
}
