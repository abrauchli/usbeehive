//! Print every USB device + Type-C port detected by the sysfs backend.
//!
//! Defaults to `/sys`. Pass an alternative root (e.g. a captured fixture
//! tree) as the first argument:
//!
//! ```sh
//! cargo run -p whatcable-sysfs --example list_devices
//! cargo run -p whatcable-sysfs --example list_devices -- /tmp/my-fixture
//! ```

use whatcable_sysfs::{DeviceManager, Sysfs};

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
        for b in &s.bullets {
            println!("    - {b}");
        }
        if let Some(d) = &s.charging_diag {
            println!("    diagnostic: {} ({:?})", d.summary, d.bottleneck);
        }
    }
}
