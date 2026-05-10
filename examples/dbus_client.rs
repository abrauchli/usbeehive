//! Tiny D-Bus client for the `usbeehived` daemon.
//!
//! ```sh
//! # one terminal
//! cargo run --no-default-features --features dbus --bin usbeehived
//!
//! # another terminal
//! cargo run --no-default-features --features dbus --example dbus_client
//! ```
//!
//! Lists every device the daemon knows about, then asks for the charging
//! diagnostic on Type-C `port_number` 0.

use usbeehive::dbus::{DeviceEntry, DiagnosticEntry, BUS_NAME, OBJECT_PATH};
use zbus::blocking::Connection;
use zbus::blocking::Proxy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session()?;
    let proxy = Proxy::new(&conn, BUS_NAME, OBJECT_PATH, "org.usbeehive.Devices1")?;

    let entries: Vec<DeviceEntry> = proxy.call("ListDevices", &())?;
    println!("== {} device(s) ==", entries.len());
    for e in &entries {
        println!(
            "[{}] {} — {}\n  port#={} icon={} bullets={}",
            e.category,
            e.headline,
            e.subtitle,
            e.port_number,
            e.icon,
            e.bullets.len(),
        );
    }

    let diag: DiagnosticEntry = proxy.call("Diagnose", &0i32)?;
    if diag.present {
        println!(
            "port 0 diagnostic: {} ({}) warning={}",
            diag.summary, diag.bottleneck, diag.is_warning
        );
        if !diag.detail.is_empty() {
            println!("  {}", diag.detail);
        }
    } else {
        println!("port 0: no diagnostic available");
    }

    Ok(())
}
