use usbkeeper_smart::selftest::{SelfTestKind, parse_state};

#[test]
fn ata_running_reports_remaining() {
    let s = parse_state(
        include_str!("fixtures/selftest-ata-running.json"),
        SelfTestKind::Short,
    )
    .unwrap();
    assert!(s.running);
    assert_eq!(s.remaining_percent, Some(90));
    assert_eq!(s.polling_minutes, Some(2));
    let long = parse_state(
        include_str!("fixtures/selftest-ata-running.json"),
        SelfTestKind::Long,
    )
    .unwrap();
    assert_eq!(long.polling_minutes, Some(110));
}

#[test]
fn ata_done_and_failed_come_from_log() {
    let ok = parse_state(
        include_str!("fixtures/selftest-ata-done.json"),
        SelfTestKind::Short,
    )
    .unwrap();
    assert!(!ok.running);
    assert_eq!(ok.log.len(), 2);
    assert_eq!(ok.log[0].kind, "Short offline");
    assert_eq!(ok.log[0].passed, Some(true));
    assert_eq!(ok.log[0].hours, Some(13451));
    let bad = parse_state(
        include_str!("fixtures/selftest-ata-failed.json"),
        SelfTestKind::Long,
    )
    .unwrap();
    assert_eq!(bad.log[0].passed, Some(false));
    assert!(bad.log[0].status.contains("read failure"));
}

#[test]
fn nvme_running_and_done() {
    let r = parse_state(
        include_str!("fixtures/selftest-nvme-running.json"),
        SelfTestKind::Short,
    )
    .unwrap();
    assert!(r.running);
    assert_eq!(r.remaining_percent, Some(70));
    let d = parse_state(
        include_str!("fixtures/selftest-nvme-done.json"),
        SelfTestKind::Short,
    )
    .unwrap();
    assert!(!d.running);
    assert_eq!(d.log[0].passed, Some(true));
    assert_eq!(d.log[0].kind, "Short");
}

#[test]
fn no_selftest_data_is_an_error() {
    assert!(parse_state("{}", SelfTestKind::Short).is_err());
    assert!(parse_state("lixo", SelfTestKind::Short).is_err());
}
