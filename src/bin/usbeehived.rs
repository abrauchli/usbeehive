//! `usbeehived` — D-Bus daemon exposing the usbeehive snapshot.
//!
//! Hosts `org.usbeehive.Devices5` on the session bus (see
//! [`usbeehive::dbus`]). A background thread runs the libudev hot-plug
//! loop, refreshes the [`DeviceManager`] under a mutex, and emits
//! `DeviceAdded` / `DeviceRemoved` / `CapabilityDegraded` /
//! `CapabilityRestored` signals as the snapshot changes.
//!
//! Run with `RUST_LOG=info` for connect/disconnect tracing on stderr.
//!
//! ```sh
//! cargo run --no-default-features --features dbus --bin usbeehived
//! gdbus call --session --dest org.usbeehive.Devices \
//!     --object-path /org/usbeehive/Devices \
//!     --method org.usbeehive.Devices5.ListDevices
//! ```
//!
//! ## systemd user service
//!
//! `usbeehived --install-service` writes a unit to
//! `$XDG_CONFIG_HOME/systemd/user/usbeehived.service` (default
//! `~/.config/systemd/user/`) with `ExecStart` set to the current binary
//! path, then runs `systemctl --user daemon-reload`. Symmetric
//! `--uninstall-service` stops, disables, and removes it.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use usbeehive::dbus::{DevicesIface, State, BUS_NAME, OBJECT_PATH};
use usbeehive::watch::{run_loop, RefreshReason};
use usbeehive::DeviceManager;

use zbus::block_on;
use zbus::blocking::connection;

const SERVICE_FILE_NAME: &str = "usbeehived.service";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => match run_daemon() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("usbeehived: {e}");
                ExitCode::FAILURE
            }
        },
        Some("--install-service") => match install_service() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("usbeehived: install failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some("--uninstall-service") => match uninstall_service() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("usbeehived: uninstall failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some("--help" | "-h") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("usbeehived {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("usbeehived: unknown argument: {other}");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "usbeehived {} — D-Bus daemon for usbeehive\n\
         \n\
         USAGE:\n\
         \x20   usbeehived                     Run the daemon (default)\n\
         \x20   usbeehived --install-service   Install systemd user unit and reload\n\
         \x20   usbeehived --uninstall-service Stop, disable, and remove the unit\n\
         \x20   usbeehived --help              Show this help\n\
         \x20   usbeehived --version           Show version",
        env!("CARGO_PKG_VERSION")
    );
}

fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("usbeehived {}: starting…", env!("CARGO_PKG_VERSION"));

    // Manager + state are shared between dispatch thread (zbus) and the
    // background hot-plug thread.
    let manager = DeviceManager::new();
    let state = Arc::new(Mutex::new(State::new(manager)));
    let iface = DevicesIface {
        state: state.clone(),
    };

    // Build the session-bus connection and register the interface.
    let connection = connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, iface)?
        .build()?;

    eprintln!(
        "usbeehived: registered {} at {} on the session bus",
        BUS_NAME, OBJECT_PATH
    );

    // Channel used by the hot-plug thread to tell main() to exit cleanly.
    let (quit_tx, quit_rx) = std::sync::mpsc::channel::<()>();
    let watcher_state = state.clone();
    let watcher_conn = connection.clone();

    thread::spawn(move || {
        let result = run_loop(Duration::from_millis(500), |reason| {
            // Snapshot diff under the lock, but don't hold it while we hop
            // into D-Bus signal emission.
            let (diff, headline_lookup): (usbeehive::SnapshotDiff, Vec<(String, String)>) = {
                let mut guard = watcher_state.lock().expect("state mutex poisoned");
                let diff = guard.refresh();
                let headlines: Vec<(String, String)> = diff
                    .added
                    .iter()
                    .map(|id| (id.clone(), guard.headline_for(id)))
                    .collect();
                (diff, headlines)
            };

            if matches!(reason, RefreshReason::Initial) {
                // Baseline-prime: don't fire signals for the devices that
                // were already plugged in when we started.
                return Ok(());
            }

            emit_signals(&watcher_conn, &diff, &headline_lookup, &watcher_state);
            let _ = io::stderr().flush();
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("usbeehived: hot-plug loop exited: {e}");
        }
        let _ = quit_tx.send(());
    });

    // Block here until the hot-plug thread bows out (SIGINT/SIGTERM via
    // `usbeehive::watch::install_default_signal_handlers`, or unrecoverable
    // libudev error).
    let _ = quit_rx.recv();
    eprintln!("usbeehived: shutting down");
    Ok(())
}

fn emit_signals(
    connection: &zbus::blocking::Connection,
    diff: &usbeehive::SnapshotDiff,
    added_headlines: &[(String, String)],
    state: &Arc<Mutex<State>>,
) {
    let object_server = connection.object_server();
    let iface_ref = match object_server.interface::<_, DevicesIface>(OBJECT_PATH) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("usbeehived: cannot resolve interface for signals: {e}");
            return;
        }
    };
    let emitter = iface_ref.signal_emitter();

    for (id, headline) in added_headlines {
        eprintln!("usbeehived: + {id} ({headline})");
        if let Err(e) = block_on(DevicesIface::device_added(
            emitter,
            id.clone(),
            headline.clone(),
        )) {
            eprintln!("usbeehived: device_added emit failed: {e}");
        }
    }

    for id in &diff.removed {
        eprintln!("usbeehived: - {id}");
        if let Err(e) = block_on(DevicesIface::device_removed(emitter, id.clone())) {
            eprintln!("usbeehived: device_removed emit failed: {e}");
        }
    }

    // For degraded ports we want the diagnostic copy — fetch it from the
    // current snapshot under the lock.
    for &port in &diff.newly_degraded {
        let (summary, detail) = {
            let guard = state.lock().expect("state mutex poisoned");
            guard
                .manager
                .devices()
                .iter()
                .find(|s| s.typec_port.as_ref().is_some_and(|p| p.port_number == port))
                .and_then(|s| {
                    s.charging_diag
                        .as_ref()
                        .map(|d| (d.summary.clone(), d.detail.clone()))
                })
                .unwrap_or_default()
        };
        eprintln!("usbeehived: ! port {port} degraded — {summary}");
        if let Err(e) = block_on(DevicesIface::capability_degraded(
            emitter, port, summary, detail,
        )) {
            eprintln!("usbeehived: capability_degraded emit failed: {e}");
        }
    }

    for &port in &diff.resolved {
        eprintln!("usbeehived: ✓ port {port} restored");
        if let Err(e) = block_on(DevicesIface::capability_restored(emitter, port)) {
            eprintln!("usbeehived: capability_restored emit failed: {e}");
        }
    }
}

fn user_unit_dir() -> Result<PathBuf, String> {
    resolve_unit_dir(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

fn resolve_unit_dir(
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, String> {
    if let Some(dir) = xdg_config_home {
        let p = PathBuf::from(dir);
        if p.is_absolute() {
            return Ok(p.join("systemd/user"));
        }
    }
    let home = home.ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config/systemd/user"))
}

fn render_unit(exec_start: &std::path::Path) -> String {
    format!(
        "[Unit]\n\
         Description=usbeehive D-Bus daemon (USB device watcher)\n\
         Documentation=https://github.com/abrauchli/usbeehive\n\
         After=dbus.socket\n\
         Requires=dbus.socket\n\
         \n\
         [Service]\n\
         Type=dbus\n\
         BusName={bus}\n\
         ExecStart={exec}\n\
         Restart=on-failure\n\
         RestartSec=2\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        bus = BUS_NAME,
        exec = exec_start.display(),
    )
}

fn install_service() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot resolve current exe: {e}"))?;
    let exe = exe
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize {}: {e}", exe.display()))?;

    let dir = user_unit_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let path = dir.join(SERVICE_FILE_NAME);

    let existed = path.exists();
    let unit = render_unit(&exe);
    std::fs::write(&path, unit).map_err(|e| format!("write {}: {e}", path.display()))?;

    eprintln!(
        "usbeehived: {} {}",
        if existed { "updated" } else { "installed" },
        path.display()
    );
    eprintln!("usbeehived:   ExecStart={}", exe.display());

    match Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
    {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("usbeehived: systemctl --user daemon-reload exited with {s}"),
        Err(e) => eprintln!("usbeehived: could not run systemctl ({e}); skip daemon-reload"),
    }

    println!("Next steps:");
    println!("  systemctl --user enable --now usbeehived.service");
    println!("  systemctl --user status usbeehived.service");
    println!("  journalctl --user -u usbeehived.service -f");
    Ok(())
}

fn uninstall_service() -> Result<(), String> {
    let dir = user_unit_dir()?;
    let path = dir.join(SERVICE_FILE_NAME);

    // Best-effort stop+disable+reset before removing. The unit may not be
    // enabled, loaded, or systemctl may be absent — suppress noise from
    // those expected "nothing to do" cases.
    let quiet = |args: &[&str]| {
        let _ = Command::new("systemctl")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    quiet(&["--user", "disable", "--now", SERVICE_FILE_NAME]);

    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        eprintln!("usbeehived: removed {}", path.display());
    } else {
        eprintln!(
            "usbeehived: no unit at {} (already uninstalled)",
            path.display()
        );
    }

    quiet(&["--user", "daemon-reload"]);
    quiet(&["--user", "reset-failed", SERVICE_FILE_NAME]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn render_unit_substitutes_exec_start() {
        let unit = render_unit(Path::new("/opt/usbeehived/usbeehived"));
        assert!(unit.contains("ExecStart=/opt/usbeehived/usbeehived\n"));
        assert!(unit.contains(&format!("BusName={BUS_NAME}\n")));
        assert!(unit.contains("Type=dbus\n"));
        assert!(unit.contains("[Install]\nWantedBy=default.target\n"));
        assert!(unit.contains("After=dbus.socket"));
    }

    #[test]
    fn resolve_unit_dir_prefers_absolute_xdg() {
        let xdg = OsStr::new("/srv/xdg");
        let home = OsStr::new("/home/u");
        let dir = resolve_unit_dir(Some(xdg), Some(home)).unwrap();
        assert_eq!(dir, PathBuf::from("/srv/xdg/systemd/user"));
    }

    #[test]
    fn resolve_unit_dir_ignores_relative_xdg() {
        let xdg = OsStr::new("relative/config");
        let home = OsStr::new("/home/u");
        let dir = resolve_unit_dir(Some(xdg), Some(home)).unwrap();
        assert_eq!(dir, PathBuf::from("/home/u/.config/systemd/user"));
    }

    #[test]
    fn resolve_unit_dir_falls_back_to_home() {
        let home = OsStr::new("/home/u");
        let dir = resolve_unit_dir(None, Some(home)).unwrap();
        assert_eq!(dir, PathBuf::from("/home/u/.config/systemd/user"));
    }

    #[test]
    fn resolve_unit_dir_errors_without_home_or_absolute_xdg() {
        assert!(resolve_unit_dir(None, None).is_err());
        assert!(resolve_unit_dir(Some(OsStr::new("rel")), None).is_err());
    }
}
