//! Integração lsblk → SafetyGuard usando uma saída real anonimizada.

use usbkeeper_core::safety::{Policy, Refusal, check_target};
use usbkeeper_core::{Device, Transport};
use usbkeeper_linux::{LinuxEnumerator, lsblk};

fn devices() -> Vec<Device> {
    lsblk::parse(include_str!("fixtures/lsblk-mixed.json")).unwrap()
}

fn by_name<'a>(devs: &'a [Device], name: &str) -> &'a Device {
    devs.iter().find(|d| d.name == name).unwrap()
}

#[test]
fn parses_disks_and_skips_partitions_at_top_level() {
    let names: Vec<_> = devices().into_iter().map(|d| d.name).collect();
    assert_eq!(
        names,
        ["loop0", "sda", "zram0", "nvme0n1", "sdb", "sdc", "sdd"]
    );
}

#[test]
fn virtual_devices_are_hidden_by_default() {
    let tmp = std::env::temp_dir(); // sysfs inexistente: sem dados USB
    let e = LinuxEnumerator {
        sys_root: tmp.join("usbkeeper-no-sys"),
        include_virtual: false,
    };
    let names: Vec<_> = e.enrich(devices()).into_iter().map(|d| d.name).collect();
    assert_eq!(names, ["sda", "nvme0n1", "sdb", "sdc", "sdd"]);
}

#[test]
fn trims_vendor_and_maps_transport() {
    let devs = devices();
    let sda = by_name(&devs, "sda");
    assert_eq!(sda.vendor, None);
    assert_eq!(by_name(&devs, "sdc").vendor.as_deref(), Some("Samsung"));
    assert_eq!(sda.transport, Transport::Sata);
    assert_eq!(sda.physical_sector, 4096);
    assert_eq!(by_name(&devs, "zram0").transport, Transport::Virtual);
    assert_eq!(
        by_name(&devs, "sdb").display_name(),
        "Kingston DataTraveler 3.0"
    );
}

#[test]
fn null_mountpoints_are_ignored() {
    let devs = devices();
    let sdb = by_name(&devs, "sdb");
    assert!(sdb.mountpoints.is_empty());
    assert_eq!(sdb.partitions[0].mountpoints, ["/media/user/PENDRIVE"]);
}

#[test]
fn accepts_legacy_string_fields() {
    let devs = devices();
    let sdd = by_name(&devs, "sdd");
    assert!(sdd.removable && sdd.hotplug && !sdd.read_only);
    assert_eq!(sdd.size_bytes, 0);
    assert_eq!(sdd.logical_sector, 512);
}

#[test]
fn guard_verdicts_over_real_topology() {
    let devs = devices();
    let p = Policy::default();

    // NVMe com / e /boot/efi: sistema, nem com --allow-non-usb.
    assert!(
        check_target(
            by_name(&devs, "nvme0n1"),
            Policy {
                allow_non_usb: true
            }
        )
        .iter()
        .any(Refusal::is_absolute)
    );

    // HD SATA de dados: só com flag, e mesmo assim montado.
    let sda = check_target(by_name(&devs, "sda"), p);
    assert!(sda.iter().any(|r| matches!(r, Refusal::NotUsb { .. })));
    assert!(sda.iter().any(|r| matches!(r, Refusal::Mounted { .. })));

    // Pendrive montado: precisa desmontar.
    assert_eq!(
        check_target(by_name(&devs, "sdb"), p),
        vec![Refusal::Mounted {
            mountpoints: vec!["/media/user/PENDRIVE".into()]
        }]
    );

    // SSD USB com root em LUKS+LVM: sistema, apesar de ser USB.
    assert_eq!(
        check_target(by_name(&devs, "sdc"), p),
        vec![Refusal::SystemDisk {
            mountpoint: "/".into()
        }]
    );

    // Leitor de cartão sem mídia.
    assert_eq!(check_target(by_name(&devs, "sdd"), p), vec![Refusal::Empty]);
}
