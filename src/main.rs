//! WhatCable-Linux CLI entry point.

use std::io;

use clap::Parser;

use whatcable_linux::manager::DeviceManager;
use whatcable_linux::output::{print_json, print_text};

#[cfg(feature = "watch")]
mod watch;

#[derive(Parser, Debug)]
#[command(
    name = "whatcable-linux",
    version,
    about = "Tells you what each USB cable/device on Linux can actually do.",
    long_about = "WhatCable-Linux — shows what each USB cable/device can do.\n\
                  Port of WhatCable (macOS) by Darryl Morley."
)]
struct Cli {
    /// Structured JSON output
    #[arg(long)]
    json: bool,

    /// Stream updates as devices change
    #[arg(long)]
    watch: bool,

    /// Include raw sysfs attributes
    #[arg(long)]
    raw: bool,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let mut mgr = DeviceManager::new();

    if cli.watch {
        #[cfg(feature = "watch")]
        return watch::run(&mut mgr, cli.json, cli.raw);
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
    } else {
        print_text(&mut out, &mgr, cli.raw)
    }
}
