//! `usbeehived` — D-Bus daemon exposing the usbeehive snapshot.
//!
//! Hosts `org.usbeehive.Devices1` on the session bus (see
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
//!     --method org.usbeehive.Devices1.ListDevices
//! ```

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use usbeehive::dbus::{DevicesIface, State, BUS_NAME, OBJECT_PATH};
use usbeehive::watch::{run_loop, RefreshReason};
use usbeehive::DeviceManager;

use zbus::block_on;
use zbus::blocking::connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            let (diff, headline_lookup): (
                usbeehive::SnapshotDiff,
                Vec<(String, String)>,
            ) = {
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
                .find(|s| {
                    s.typec_port
                        .as_ref()
                        .is_some_and(|p| p.port_number == port)
                })
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
