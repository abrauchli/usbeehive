//! Thin helpers over `/sys` attribute reads.
//!
//! sysfs files are tiny line-oriented pseudo-files; we trim trailing whitespace
//! and parse on demand. Anything missing or malformed yields `None` rather than
//! an error — callers treat absence as "this attribute isn't available on this
//! kernel/hardware combination."

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Read a sysfs attribute as a trimmed string. Returns `None` if the file is
/// missing, unreadable, or empty.
pub fn read_attr(path: impl AsRef<Path>) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read a base-10 integer attribute.
pub fn read_int(path: impl AsRef<Path>) -> Option<i64> {
    read_attr(path)?.parse().ok()
}

/// Read a hex attribute (with or without a `0x` prefix).
pub fn read_hex(path: impl AsRef<Path>) -> Option<u32> {
    let s = read_attr(path)?;
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(&s);
    u32::from_str_radix(stripped, 16).ok()
}

pub fn path_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}

/// List immediate subdirectories of `path`, sorted by name. Returns an empty
/// vec when the directory is missing or unreadable.
pub fn subdirs(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(path) else {
        return Vec::new();
    };
    // sysfs entries are symlinks to real device directories; follow them.
    let mut out: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// Read every regular file in `dir` as a sysfs attribute, keyed by filename.
/// Used for the `--raw` view; silently skips unreadable entries.
pub fn read_all_attrs(dir: impl AsRef<Path>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(rd) = fs::read_dir(&dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(val) = read_attr(entry.path()) {
            out.insert(name, val);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp() -> tempdir_lite::TmpDir {
        tempdir_lite::TmpDir::new("wcsys")
    }

    fn write(p: &Path, s: &str) {
        let mut f = fs::File::create(p).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[test]
    fn reads_and_trims_attr() {
        let d = tmp();
        let p = d.path().join("a");
        write(&p, "  hello\n");
        assert_eq!(read_attr(&p).as_deref(), Some("hello"));
    }

    #[test]
    fn empty_attr_yields_none() {
        let d = tmp();
        let p = d.path().join("a");
        write(&p, "\n");
        assert!(read_attr(&p).is_none());
    }

    #[test]
    fn hex_strips_prefix() {
        let d = tmp();
        let p = d.path().join("h");
        write(&p, "0x1A2b\n");
        assert_eq!(read_hex(&p), Some(0x1A2B));
        write(&p, "ff\n");
        assert_eq!(read_hex(&p), Some(0xFF));
    }

    #[test]
    fn int_parses_decimal() {
        let d = tmp();
        let p = d.path().join("i");
        write(&p, "480\n");
        assert_eq!(read_int(&p), Some(480));
    }

    #[test]
    fn missing_paths_are_none() {
        assert!(read_attr("/no/such/path/whatcable").is_none());
        assert!(read_int("/no/such/path/whatcable").is_none());
        assert!(read_hex("/no/such/path/whatcable").is_none());
        assert!(!path_exists("/no/such/path/whatcable"));
    }

    #[test]
    fn read_all_attrs_collects_files() {
        let d = tmp();
        write(&d.path().join("speed"), "480");
        write(&d.path().join("idVendor"), "05ac");
        let attrs = read_all_attrs(d.path());
        assert_eq!(attrs.get("speed").map(String::as_str), Some("480"));
        assert_eq!(attrs.get("idVendor").map(String::as_str), Some("05ac"));
    }
}

#[cfg(test)]
mod tempdir_lite {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    pub struct TmpDir {
        path: PathBuf,
    }

    impl TmpDir {
        pub fn new(prefix: &str) -> Self {
            let n = N.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("{prefix}-{pid}-{n}"));
            fs::create_dir_all(&path).unwrap();
            TmpDir { path }
        }
        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
