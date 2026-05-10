//! Print a one-line message every time the udev `usb` / `typec`
//! subsystems emit a change.
//!
//! ```sh
//! cargo run -p usbeehive-watch --example print_changes
//! ```
//!
//! Plug or unplug a USB device to trigger output. Press `Ctrl-C` to exit.

use std::time::{Duration, Instant};

use usbeehive::watch::{run_loop, RefreshReason};

fn main() {
    let started = Instant::now();
    run_loop(Duration::from_millis(250), |reason| {
        let elapsed = started.elapsed().as_millis();
        match reason {
            RefreshReason::Initial => {
                println!("[+{elapsed}ms] watch started");
            }
            RefreshReason::Hotplug => {
                println!("[+{elapsed}ms] usb / typec change");
            }
        }
        Ok(())
    })
    .expect("run_loop");
}
