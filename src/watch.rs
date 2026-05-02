//! `--watch` mode: re-render whenever udev notifies us of USB/typec changes,
//! debounced so a burst of events triggers exactly one refresh.

use std::io::{self, Write};
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use whatcable::manager::DeviceManager;
use whatcable::monitor::UdevMonitor;
use whatcable::output::{print_json, print_text};

static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn on_signal(_: libc::c_int) {
    RUNNING.store(false, Ordering::SeqCst);
}

pub fn run(mgr: &mut DeviceManager, use_json: bool, show_raw: bool) -> io::Result<()> {
    RUNNING.store(true, Ordering::SeqCst);
    unsafe {
        libc::signal(libc::SIGINT, on_signal as usize);
        libc::signal(libc::SIGTERM, on_signal as usize);
    }

    let mut monitor = UdevMonitor::new()?;
    mgr.refresh();
    render(mgr, use_json, show_raw)?;

    let fd = monitor.fd();
    let debounce = Duration::from_millis(500);
    let mut dirty: Option<Instant> = None;

    while RUNNING.load(Ordering::SeqCst) {
        let timeout_ms: i32 = match dirty {
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32,
            None => -1,
        };

        match poll_one(fd, timeout_ms) {
            Ready::Readable => {
                monitor.drain();
                dirty = Some(Instant::now() + debounce);
            }
            Ready::Timeout => {}
            Ready::Interrupted => continue,
        }

        if let Some(deadline) = dirty {
            if Instant::now() >= deadline {
                mgr.refresh();
                render(mgr, use_json, show_raw)?;
                dirty = None;
            }
        }
    }
    Ok(())
}

fn render(mgr: &DeviceManager, use_json: bool, show_raw: bool) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if !use_json {
        // Clear screen + home cursor.
        write!(out, "\x1b[2J\x1b[H")?;
        print_text(&mut out, mgr, show_raw)?;
    } else {
        print_json(&mut out, mgr, show_raw)?;
    }
    out.flush()
}

enum Ready {
    Readable,
    Timeout,
    Interrupted,
}

fn poll_one(fd: RawFd, timeout_ms: i32) -> Ready {
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let r = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if r < 0 {
        Ready::Interrupted
    } else if r == 0 {
        Ready::Timeout
    } else if (pfd.revents & libc::POLLIN) != 0 {
        Ready::Readable
    } else {
        Ready::Timeout
    }
}
