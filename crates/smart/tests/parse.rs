use usbkeeper_smart::{HealthVerdict, parse};

#[test]
fn ata_behind_usb_bridge_with_reallocated_sectors_is_warning() {
    let r = parse(include_str!("fixtures/ata-usb-sat-worn.json"));
    assert_eq!(r.verdict, HealthVerdict::Warning);
    assert_eq!(r.device_type.as_deref(), Some("sat"));
    assert_eq!(r.reallocated_sectors, Some(8));
    assert_eq!(r.pending_sectors, Some(0));
    assert_eq!(r.power_on_hours, Some(13450));
    assert_eq!(r.power_cycles, Some(2210));
    assert_eq!(r.temperature_c, Some(36));
    assert_eq!(r.attributes.len(), 7);
    assert!(r.notes.iter().any(|n| n.contains("8 setores realocados")));
}

#[test]
fn healthy_nvme_behind_usb_is_good() {
    let r = parse(include_str!("fixtures/nvme-good.json"));
    assert_eq!(r.verdict, HealthVerdict::Good);
    assert_eq!(r.protocol.as_deref(), Some("NVMe"));
    assert_eq!(r.percentage_used, Some(3));
    assert_eq!(r.media_errors, Some(0));
    assert!(r.notes.is_empty());
}

#[test]
fn failed_overall_and_attribute_now_is_failing() {
    let r = parse(include_str!("fixtures/ata-failing.json"));
    assert_eq!(r.verdict, HealthVerdict::Failing);
    assert_eq!(r.passed, Some(false));
    assert!(r.notes[0].contains("reprovou"));
    assert!(r.notes.iter().any(|n| n.contains("atributo 5")));
    assert!(r.notes.iter().any(|n| n.contains("CRC")));
}

#[test]
fn unknown_bridge_explains_why() {
    let r = parse(include_str!("fixtures/usb-unknown-bridge.json"));
    assert_eq!(r.verdict, HealthVerdict::Unknown);
    assert!(r.notes[0].contains("adaptador USB não repassa"));
    assert!(!r.access_denied);
}

#[test]
fn permission_denied_suggests_sudo() {
    let r = parse(include_str!("fixtures/permission-denied.json"));
    assert_eq!(r.verdict, HealthVerdict::Unknown);
    assert!(r.notes[0].contains("sudo"));
    assert!(r.access_denied);
}

#[test]
fn garbage_is_unknown_not_panic() {
    assert_eq!(parse("não é json").verdict, HealthVerdict::Unknown);
    assert_eq!(parse("{}").verdict, HealthVerdict::Unknown);
}
