//! Diff two sysfs snapshots: which `bus_port`s appeared, disappeared, or
//! changed. Useful for capturing a "before vs after plug-in" comparison
//! against a fixture tree.
//!
//! ```sh
//! cargo run -p whatcable-sysfs --example snapshot_diff -- /tmp/before /tmp/after
//! ```

use std::collections::HashSet;

use whatcable_sysfs::{DeviceManager, Sysfs};

fn collect(root: &str) -> Vec<(String, String)> {
    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root));
    mgr.refresh();
    mgr.usb_devices()
        .iter()
        .filter(|d| !d.is_root_hub)
        .map(|d| (d.bus_port.clone(), d.display_name()))
        .collect()
}

fn main() {
    let before = std::env::args().nth(1).expect("before-root");
    let after = std::env::args().nth(2).expect("after-root");

    let a: Vec<_> = collect(&before);
    let b: Vec<_> = collect(&after);

    let a_set: HashSet<_> = a.iter().map(|x| x.0.as_str()).collect();
    let b_set: HashSet<_> = b.iter().map(|x| x.0.as_str()).collect();

    println!("=== removed ===");
    for (port, name) in &a {
        if !b_set.contains(port.as_str()) {
            println!("  - {port}: {name}");
        }
    }
    println!("=== added ===");
    for (port, name) in &b {
        if !a_set.contains(port.as_str()) {
            println!("  + {port}: {name}");
        }
    }
}
