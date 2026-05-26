//! Thunderbolt / USB4 router enumeration from `/sys/bus/thunderbolt/devices/`.

use std::path::{Path, PathBuf};

use crate::thunderbolt::ThunderboltRouter;

use super::reader::{self, Sysfs};

impl Sysfs {
    /// Walk this sysfs root's Thunderbolt devices directory and return every
    /// router (host adapters and connected devices). Empty `Vec` when the
    /// subsystem isn't loaded.
    pub fn thunderbolt_routers(&self) -> Vec<ThunderboltRouter> {
        enumerate_in(&self.thunderbolt_dir())
    }
}

fn from_sysfs(path: &Path, name: &str) -> Option<ThunderboltRouter> {
    // Route format: `<domain>-<segment>[-<segment>...]`. Strip any non-router
    // entries first (the bus directory exposes service nodes too — they
    // don't carry a `generation` attribute).
    let (domain_str, _rest) = name.split_once('-')?;
    let domain: u32 = domain_str.parse().ok()?;
    let generation = reader::read_int(path.join("generation")).map(|v| v.clamp(0, 255) as u8)?;
    if generation == 0 {
        return None;
    }

    // Host adapter route always terminates in `-0` (`0-0`, `1-0`, …).
    let is_host = name.rsplit('-').next() == Some("0");

    let vendor_id = reader::read_hex(path.join("vendor"))
        .map(|v| (v & 0xFFFF) as u16)
        .unwrap_or(0);
    let device_id = reader::read_hex(path.join("device"))
        .map(|v| (v & 0xFFFF) as u16)
        .unwrap_or(0);

    Some(ThunderboltRouter {
        sysfs_path: path.to_path_buf(),
        route: name.to_string(),
        domain,
        is_host,
        generation,
        vendor_id,
        device_id,
        vendor_name: reader::read_attr(path.join("vendor_name")).unwrap_or_default(),
        device_name: reader::read_attr(path.join("device_name")).unwrap_or_default(),
        unique_id: reader::read_attr(path.join("unique_id")).unwrap_or_default(),
        raw_attributes: reader::read_all_attrs(path),
    })
}

pub(crate) fn enumerate_in(base: &Path) -> Vec<ThunderboltRouter> {
    if !reader::path_exists(base) {
        return Vec::new();
    }
    let mut entries: Vec<(PathBuf, String)> = reader::subdirs(base)
        .into_iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            // Skip subsystem links (`domain0`, etc.) and anything that
            // doesn't look like a route string.
            if !name.contains('-') {
                return None;
            }
            Some((p, name))
        })
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    entries
        .iter()
        .filter_map(|(p, n)| from_sysfs(p, n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write_file(p: &Path, s: &str) {
        let mut f = fs::File::create(p).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    fn write_router(base: &Path, route: &str, generation: u8, extras: &[(&str, &str)]) {
        let dir = base.join(route);
        fs::create_dir_all(&dir).unwrap();
        write_file(&dir.join("generation"), &generation.to_string());
        for (k, v) in extras {
            write_file(&dir.join(k), v);
        }
    }

    #[test]
    fn missing_thunderbolt_dir_returns_empty() {
        assert!(enumerate_in(Path::new("/no/such/usbeehive/path")).is_empty());
    }

    #[test]
    fn enumerates_host_and_device_routers() {
        let d = super::super::reader::tempdir_lite::TmpDir::new("tb-enum");
        write_router(d.path(), "0-0", 4, &[("vendor_name", "Intel"), ("device_name", "Maple Ridge")]);
        write_router(d.path(), "0-1", 4, &[("vendor_name", "OWC"), ("unique_id", "abcd")]);
        write_router(d.path(), "0-1-1", 3, &[]);
        // Subsystem link without a generation file → ignored.
        fs::create_dir_all(d.path().join("domain0")).unwrap();

        let routers = enumerate_in(d.path());
        assert_eq!(routers.len(), 3);

        let host = routers.iter().find(|r| r.route == "0-0").unwrap();
        assert!(host.is_host);
        assert_eq!(host.generation, 4);
        assert_eq!(host.vendor_name, "Intel");

        let dev = routers.iter().find(|r| r.route == "0-1").unwrap();
        assert!(!dev.is_host);
        assert_eq!(dev.unique_id, "abcd");

        let nested = routers.iter().find(|r| r.route == "0-1-1").unwrap();
        assert!(!nested.is_host);
        assert_eq!(nested.generation, 3);
    }

    #[test]
    fn routers_without_generation_are_skipped() {
        let d = super::super::reader::tempdir_lite::TmpDir::new("tb-no-gen");
        fs::create_dir_all(d.path().join("0-0")).unwrap();
        // No `generation` file written.
        let routers = enumerate_in(d.path());
        assert!(routers.is_empty());
    }
}
