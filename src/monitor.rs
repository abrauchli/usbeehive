//! libudev hotplug monitor for the `usb` and `typec` subsystems.
//!
//! Wraps the `udev` crate's `MonitorSocket`. The CLI polls the raw fd and
//! drains events via `drain()` whenever the socket is readable.

use std::io;
use std::os::fd::{AsRawFd, RawFd};

use udev::MonitorBuilder;

pub struct UdevMonitor {
    socket: udev::MonitorSocket,
}

impl UdevMonitor {
    pub fn new() -> io::Result<Self> {
        let socket = MonitorBuilder::new()?
            .match_subsystem("usb")?
            .match_subsystem("typec")?
            .listen()?;
        Ok(UdevMonitor { socket })
    }

    pub fn fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    /// Consume any pending events. Returns the number drained — useful as a
    /// "did anything actually change?" signal, though we still re-enumerate
    /// from sysfs after any non-zero drain.
    pub fn drain(&mut self) -> usize {
        self.socket.iter().count()
    }
}
