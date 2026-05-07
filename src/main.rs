//! WhatCable CLI entry point.

use std::io::{self, Write};

use clap::Parser;

use whatcable::{DeviceManager, Sysfs};

mod output;

use output::{print_json, print_text, print_tree};

#[derive(Parser, Debug)]
#[command(
    name = "whatcable",
    version,
    about = "Tells you what each USB cable / device on Linux can actually do.",
    long_about = "WhatCable — shows what each USB cable / device can do.\n\
                  Port of WhatCable (macOS) by Darryl Morley."
)]
struct Cli {
    /// Structured JSON output.
    #[arg(long)]
    json: bool,

    /// Render a flat list with full per-device details (default is a topology tree).
    #[arg(long, conflicts_with = "tree")]
    list: bool,

    /// Render the bus topology as a tree (default).
    #[arg(long, conflicts_with = "list")]
    tree: bool,

    /// Stream updates as devices change (requires the `watch` feature).
    #[arg(long)]
    watch: bool,

    /// Include raw sysfs attributes in the output.
    #[arg(long)]
    raw: bool,

    /// Override the sysfs root (default: /sys). Useful for fixture-based testing.
    #[arg(long, value_name = "PATH")]
    sysfs_root: Option<std::path::PathBuf>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let sysfs = match cli.sysfs_root.as_ref() {
        Some(p) => Sysfs::with_root(p),
        None => Sysfs::linux(),
    };
    let mut mgr = DeviceManager::with_sysfs(sysfs);

    let use_list = cli.list;
    if cli.watch {
        #[cfg(feature = "watch")]
        return run_watch(&mut mgr, cli.json, use_list, cli.raw);
        #[cfg(not(feature = "watch"))]
        {
            eprintln!("--watch is not available: built without the `watch` feature.");
            std::process::exit(2);
        }
    }
    mgr.refresh();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if cli.json {
        print_json(&mut out, &mgr, cli.raw)
    } else if use_list {
        print_text(&mut out, &mgr, cli.raw)
    } else {
        print_tree(&mut out, &mgr)
    }
}

#[cfg(feature = "watch")]
fn run_watch(
    mgr: &mut DeviceManager,
    use_json: bool,
    use_list: bool,
    show_raw: bool,
) -> io::Result<()> {
    use std::time::Duration;
    use whatcable::watch::{run_loop, RefreshReason};

    run_loop(Duration::from_millis(500), |reason| {
        mgr.refresh();
        let stdout = io::stdout();
        let mut out = stdout.lock();
        if !use_json {
            // Clear screen + home cursor between renders so the latest snapshot
            // is always visible at the top of the terminal.
            if matches!(reason, RefreshReason::Hotplug | RefreshReason::Initial) {
                write!(out, "\x1b[2J\x1b[H")?;
            }
            if use_list {
                print_text(&mut out, mgr, show_raw)?;
            } else {
                print_tree(&mut out, mgr)?;
            }
        } else {
            print_json(&mut out, mgr, show_raw)?;
        }
        out.flush()
    })
}
