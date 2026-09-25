#[path = "../src/platform/macos/atdd_control.rs"]
mod atdd_control;
#[path = "../src/platform/macos/hotkey_state.rs"]
mod hotkey_state;

use atdd_control::AtddControl;
use hotkey_state::{Action, Machine, Mode, Snapshot};

#[test]
fn assistant_stays_recording_until_driver_releases_it() {
    let mut machine = Machine::default();
    let mut driver = AtddControl::default();
    let (id, action) = driver.begin(&mut machine, Mode::Assistant).unwrap();
    assert_eq!(action, Action::Start(Mode::Assistant));
    for _ in 0..100 {
        let snapshot = driver.snapshot(Snapshot::default(), true);
        assert_eq!(machine.tick(snapshot, true, false, false), None);
    }
    assert_eq!(
        driver.finish(&mut machine, id),
        Some(Action::Stop(Mode::Assistant))
    );
    assert_eq!(driver.finish(&mut machine, id), None);
}

#[test]
fn dictation_keeps_release_mode_and_finishes_once() {
    let mut machine = Machine::default();
    let mut driver = AtddControl::default();
    let (id, action) = driver.begin(&mut machine, Mode::Release).unwrap();
    assert_eq!(action, Action::Start(Mode::Release));
    assert_eq!(
        machine.tick(
            driver.snapshot(Snapshot::default(), true),
            true,
            false,
            false
        ),
        None
    );
    assert_eq!(
        driver.finish(&mut machine, id),
        Some(Action::Stop(Mode::Release))
    );
    assert!(machine.recording.is_none());
}

#[test]
fn cancelled_driver_cannot_stop_a_later_physical_recording() {
    let mut machine = Machine::default();
    let mut driver = AtddControl::default();
    let (old, _) = driver.begin(&mut machine, Mode::Assistant).unwrap();
    driver.reset();
    machine.reset();
    machine.tick(Snapshot::default(), true, false, false);
    assert_eq!(
        machine.tick(
            Snapshot {
                assistant: true,
                ..Snapshot::default()
            },
            true,
            false,
            false
        ),
        Some(Action::Start(Mode::Assistant))
    );
    assert_eq!(driver.finish(&mut machine, old), None);
    assert_eq!(machine.recording, Some(Mode::Assistant));
}

#[test]
fn stale_driver_id_cannot_stop_new_driver() {
    let mut machine = Machine::default();
    let mut driver = AtddControl::default();
    let (old, _) = driver.begin(&mut machine, Mode::Assistant).unwrap();
    driver.finish(&mut machine, old);
    let (new, _) = driver.begin(&mut machine, Mode::Release).unwrap();
    assert_ne!(old, new);
    assert_eq!(driver.finish(&mut machine, old), None);
    assert!(driver.owns(new));
    assert_eq!(machine.recording, Some(Mode::Release));
}

#[test]
fn permission_loss_stops_the_correct_mode_and_invalidates_driver() {
    let mut machine = Machine::default();
    let mut driver = AtddControl::default();
    let (id, _) = driver.begin(&mut machine, Mode::Assistant).unwrap();
    let snapshot = driver.snapshot(Snapshot::default(), false);
    assert_eq!(
        machine.tick(snapshot, false, false, false),
        Some(Action::Stop(Mode::Assistant))
    );
    assert_eq!(driver.finish(&mut machine, id), None);
    assert!(!driver.owns(id));
}

#[test]
fn busy_machine_is_never_replaced_by_test_driver() {
    let mut machine = Machine::default();
    let mut driver = AtddControl::default();
    machine.tick(
        Snapshot {
            dictation: true,
            ..Snapshot::default()
        },
        true,
        false,
        false,
    );
    assert!(driver.begin(&mut machine, Mode::Assistant).is_err());
    assert_eq!(machine.recording, Some(Mode::Dictation));
}
