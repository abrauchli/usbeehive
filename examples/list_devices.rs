//! Print every USB device + Type-C port detected by the sysfs backend.
//!
//! Defaults to `/sys`. Pass an alternative root (e.g. a captured fixture
//! tree) as the first argument:
//!
//! ```sh
//! cargo run -p usbeehive-sysfs --example list_devices
//! cargo run -p usbeehive-sysfs --example list_devices -- /tmp/my-fixture
//! ```

use usbeehive::{DeviceManager, Sysfs};

fn main() {
    let sysfs = match std::env::args().nth(1) {
        Some(p) => Sysfs::with_root(p),
        None => Sysfs::linux(),
    };
    let mut mgr = DeviceManager::with_sysfs(sysfs);
    mgr.refresh();

    let snap = mgr.snapshot();
    println!("USB devices       : {}", snap.usb_devices.len());
    println!("Type-C ports      : {}", snap.typec_ports.len());
    println!("USB-PD ports      : {}", snap.pd_ports.len());
    println!("Renderable items  : {}", snap.summaries.len());
    println!();

    for s in &snap.summaries {
        println!("• {}", s.headline);
        if !s.subtitle.is_empty() {
            println!("    {}", s.subtitle);
        }
        if s.link_speed_mbps > 0 {
            println!("    - link speed: {} Mbps", s.link_speed_mbps);
        }
        if !s.usb_version.is_empty() {
            println!("    - USB {}", s.usb_version);
        }
        for (k, v) in &s.properties {
            println!("    - {k}: {v}");
        }
        if let Some(d) = &s.charging_diag {
            println!("    diagnostic: {} ({:?})", d.summary, d.bottleneck);
        }
    }
}
