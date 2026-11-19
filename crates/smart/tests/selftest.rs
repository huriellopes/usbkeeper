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

use usbkeeper_smart::selftest::{SelfTestEntry, SelfTestState, new_result};

fn entry(kind: &str, hours: u64) -> SelfTestEntry {
    SelfTestEntry {
        kind: kind.into(),
        status: "Completed without error".into(),
        passed: Some(true),
        hours: Some(hours),
    }
}

fn state(log: Vec<SelfTestEntry>, last_passed: Option<bool>) -> SelfTestState {
    SelfTestState {
        running: false,
        remaining_percent: None,
        polling_minutes: None,
        log,
        last_passed,
    }
}

#[test]
fn identical_entry_in_same_hour_is_still_new_when_log_shifts() {
    // Dois testes curtos na mesma hora de uso: a entrada nova é idêntica à anterior.
    let before = vec![
        entry("Short offline", 13451),
        entry("Extended offline", 12000),
    ];
    let after = state(
        vec![
            entry("Short offline", 13451),
            entry("Short offline", 13451),
            entry("Extended offline", 12000),
        ],
        Some(true),
    );
    assert_eq!(
        new_result(&before, &after).and_then(|e| e.passed),
        Some(true)
    );
}

#[test]
fn full_log_shifts_without_growing() {
    let before = vec![
        entry("Short offline", 10),
        entry("Short offline", 9),
        entry("Short offline", 8),
    ];
    let after = state(
        vec![
            entry("Short offline", 10),
            entry("Short offline", 10),
            entry("Short offline", 9),
        ],
        None,
    );
    assert!(new_result(&before, &after).is_some());
}

#[test]
fn unchanged_log_is_not_a_new_result_but_disk_status_is_used() {
    let before = vec![entry("Short offline", 10), entry("Extended offline", 9)];
    let unchanged = state(before.clone(), None);
    assert!(new_result(&before, &unchanged).is_none());
    // Sem entrada nova mas com o status do último teste informado pelo disco.
    let with_status = state(before.clone(), Some(false));
    assert_eq!(
        new_result(&before, &with_status).and_then(|e| e.passed),
        Some(false)
    );
}

#[test]
fn ata_fixture_exposes_last_status() {
    let s = parse_state(
        include_str!("fixtures/selftest-ata-done.json"),
        SelfTestKind::Short,
    )
    .unwrap();
    assert_eq!(s.last_passed, Some(true));
    let f = parse_state(
        include_str!("fixtures/selftest-ata-failed.json"),
        SelfTestKind::Long,
    )
    .unwrap();
    assert_eq!(f.last_passed, Some(false));
}
