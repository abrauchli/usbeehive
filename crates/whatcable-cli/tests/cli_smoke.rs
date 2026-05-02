//! End-to-end smoke tests for the `whatcable` binary.
//!
//! Builds a tiny sysfs tree on disk, invokes the binary with `--sysfs-root`
//! pointed at it, and asserts on the stdout. Lifts coverage from "code
//! compiles" up to "the program runs and produces the right output".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("wccli-{label}-{pid}-{n}"));
        fs::create_dir_all(&path).unwrap();
        TempRoot { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_attr(dir: &Path, name: &str, value: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), value).unwrap();
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_whatcable"))
}

fn write_keyboard_fixture(root: &Path) {
    let usb_dir = root.join("bus/usb/devices/1-1");
    fs::create_dir_all(&usb_dir).unwrap();
    write_attr(&usb_dir, "idVendor", "046d");
    write_attr(&usb_dir, "idProduct", "c31c");
    write_attr(&usb_dir, "product", "USB Keyboard");
    write_attr(&usb_dir, "manufacturer", "Logitech");
    write_attr(&usb_dir, "speed", "12");
    write_attr(&usb_dir, "bMaxPower", "100mA");
    write_attr(&usb_dir, "version", "2.00");
    write_attr(&usb_dir, "bDeviceClass", "00");
    write_attr(&usb_dir, "busnum", "1");
    write_attr(&usb_dir, "devnum", "2");
    write_attr(&usb_dir, "removable", "removable");
    write_attr(&usb_dir, "bNumInterfaces", "1");
    let if_dir = usb_dir.join("1-1:1.0");
    fs::create_dir_all(&if_dir).unwrap();
    write_attr(&if_dir, "bInterfaceClass", "03");
    write_attr(&if_dir, "bInterfaceSubClass", "01");
    write_attr(&if_dir, "bInterfaceProtocol", "01");
}

#[test]
fn empty_root_says_no_devices() {
    let root = TempRoot::new("empty");
    let out = cli()
        .arg("--sysfs-root")
        .arg(root.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No USB devices found."),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn keyboard_renders_in_text_mode() {
    let root = TempRoot::new("kb-text");
    write_keyboard_fixture(root.path());

    let out = cli()
        .arg("--sysfs-root")
        .arg(root.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USB Keyboard"));
    assert!(stdout.contains("Logitech"));
    assert!(stdout.contains("HID"));
    assert!(stdout.contains("VID:PID 046d:c31c"));
}

#[test]
fn keyboard_renders_in_json_mode() {
    let root = TempRoot::new("kb-json");
    write_keyboard_fixture(root.path());

    let out = cli()
        .arg("--json")
        .arg("--sysfs-root")
        .arg(root.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    let obj = &arr[0];
    assert_eq!(obj["category"], "usb");
    assert_eq!(obj["headline"], "USB Keyboard");
    assert_eq!(obj["usb"]["vendorId"], "0x046d");
    assert_eq!(obj["usb"]["speed"], 12);
}

#[test]
fn raw_flag_includes_sysfs_attributes() {
    let root = TempRoot::new("kb-raw");
    write_keyboard_fixture(root.path());

    let out = cli()
        .arg("--json")
        .arg("--raw")
        .arg("--sysfs-root")
        .arg(root.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed[0]["usb"]["raw"]["idVendor"].as_str() == Some("046d"));
    assert!(parsed[0]["usb"]["raw"]["product"].as_str() == Some("USB Keyboard"));
}

#[test]
fn version_flag_works() {
    let out = cli().arg("--version").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("whatcable"));
}

#[test]
fn help_flag_works() {
    let out = cli().arg("--help").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("--sysfs-root"));
}
