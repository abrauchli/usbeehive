# whatcable-cli

Command-line interface for [WhatCable](https://github.com/Zetaphor/whatcable-linux).
Tells you what each USB cable / device on Linux can actually do.

```bash
cargo install whatcable-cli
whatcable             # human-readable summary of every USB device
whatcable --json      # structured JSON output
whatcable --watch     # stream updates as devices come and go
whatcable --raw       # include raw sysfs attributes
whatcable --sysfs-root /path/to/captured/tree    # for fixture testing
```

## Build features

- `watch` (default) — enables `--watch` via `whatcable-watch`. Disable with
  `--no-default-features` to drop the `libudev` link dependency.

## License

[MIT](../../LICENSE)
