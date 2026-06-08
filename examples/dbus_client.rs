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
//! Lists every device the daemon knows about, exercising every
//! structured field on the Devices4 wire (`device_class`, `usb_version`,
//! `link_speed_mbps`, `power`, `charging_diag`, `properties`), then
//! calls `Diagnose(0)` to show the per-port lookup still returns the
//! same data on its own.

use usbeehive::dbus::{DeviceEntry, DiagnosticEntry, BUS_NAME, OBJECT_PATH};
use zbus::blocking::Connection;
use zbus::blocking::Proxy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session()?;
    let proxy = Proxy::new(&conn, BUS_NAME, OBJECT_PATH, "org.usbeehive.Devices4")?;

    let entries: Vec<DeviceEntry> = proxy.call("ListDevices", &())?;
    println!("== {} device(s) ==", entries.len());
    for e in &entries {
        println!(
            "[{} / {}{}] {}",
            e.category,
            e.device_class,
            if e.device_subclass.is_empty() {
                String::new()
            } else {
                format!("/{}", e.device_subclass)
            },
            e.headline,
        );
        if !e.subtitle.is_empty() {
            println!("  {}", e.subtitle);
        }
        println!(
            "  status={} icon={} port#={} vid:pid={:04x}:{:04x}",
            e.status, e.icon, e.port_number, e.vendor_id, e.product_id,
        );
        if !e.primary_driver.is_empty() {
            println!("  driver: {}", e.primary_driver);
        }
        if e.link_speed_mbps > 0 || !e.usb_version.is_empty() {
            println!("  link: USB {} @ {} Mbps", e.usb_version, e.link_speed_mbps);
        }
        if e.power.power_in_mw > 0 || e.power.power_out_mw > 0 {
            println!(
                "  power: in={} mW out={} mW role={}",
                e.power.power_in_mw, e.power.power_out_mw, e.power.power_role,
            );
        }
        if e.charging_diag.present {
            println!(
                "  charging: {} ({}) warning={}",
                e.charging_diag.summary, e.charging_diag.bottleneck, e.charging_diag.is_warning,
            );
            if !e.charging_diag.detail.is_empty() {
                println!("    {}", e.charging_diag.detail);
            }
        }
        for (k, v) in &e.properties {
            println!("  {k}: {v}");
        }
    }

    let diag: DiagnosticEntry = proxy.call("Diagnose", &0i32)?;
    if diag.present {
        println!(
            "Diagnose(0): {} ({}) warning={}",
            diag.summary, diag.bottleneck, diag.is_warning
        );
        if !diag.detail.is_empty() {
            println!("  {}", diag.detail);
        }
    } else {
        println!("Diagnose(0): no diagnostic available");
    }

    Ok(())
}
