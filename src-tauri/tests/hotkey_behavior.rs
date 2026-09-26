#[path = "../src/platform/hotkey_state.rs"]
mod hotkey_state;

use hotkey_state::{Action, Machine, Mode, Snapshot};

fn pressed(mode: Mode) -> Snapshot {
    Snapshot {
        dictation: mode == Mode::Dictation,
        assistant: mode == Mode::Assistant,
        release: mode == Mode::Release,
    }
}

#[test]
fn hold_modes_emit_one_start_and_one_stop_despite_repeated_samples() {
    for mode in [Mode::Dictation, Mode::Assistant] {
        let mut machine = Machine::default();
        assert_eq!(
            machine.tick(pressed(mode), true, false, false),
            Some(Action::Start(mode))
        );
        for _ in 0..20 {
            assert_eq!(machine.tick(pressed(mode), true, false, false), None);
        }
        assert_eq!(
            machine.tick(Snapshot::default(), true, false, false),
            Some(Action::Stop(mode))
        );
        assert_eq!(machine.tick(Snapshot::default(), true, false, false), None);
        assert_eq!(machine.recording, None);
    }
}

#[test]
fn toggle_configuration_is_independent_for_each_mode() {
    for mode in [Mode::Dictation, Mode::Assistant] {
        let mut machine = Machine::default();
        let dictation_toggle = mode == Mode::Dictation;
        let assistant_toggle = mode == Mode::Assistant;
        assert_eq!(
            machine.tick(pressed(mode), true, dictation_toggle, assistant_toggle),
            Some(Action::Start(mode))
        );
        assert_eq!(
            machine.tick(pressed(mode), true, dictation_toggle, assistant_toggle),
            None
        );
        assert_eq!(
            machine.tick(
                Snapshot::default(),
                true,
                dictation_toggle,
                assistant_toggle
            ),
            None
        );
        assert_eq!(machine.recording, Some(mode));
        assert_eq!(
            machine.tick(pressed(mode), true, dictation_toggle, assistant_toggle),
            Some(Action::Stop(mode))
        );
    }
}

#[test]
fn dictation_wins_over_assistant_on_simultaneous_press() {
    let mut machine = Machine::default();
    let both = Snapshot {
        dictation: true,
        assistant: true,
        release: false,
    };
    assert_eq!(
        machine.tick(both, true, false, false),
        Some(Action::Start(Mode::Dictation))
    );
    // Stopping the winning mode must not start the other still-held shortcut.
    assert_eq!(
        machine.tick(pressed(Mode::Assistant), true, false, false),
        Some(Action::Stop(Mode::Dictation))
    );
    assert_eq!(
        machine.tick(pressed(Mode::Assistant), true, false, false),
        None
    );
    machine.tick(Snapshot::default(), true, false, false);
    assert_eq!(
        machine.tick(pressed(Mode::Assistant), true, false, false),
        Some(Action::Start(Mode::Assistant))
    );
}

#[test]
fn another_mode_cannot_steal_a_toggle_recording() {
    for (owner, other) in [
        (Mode::Dictation, Mode::Assistant),
        (Mode::Assistant, Mode::Dictation),
    ] {
        let mut machine = Machine::default();
        machine.tick(pressed(owner), true, true, true);
        machine.tick(Snapshot::default(), true, true, true);
        assert_eq!(machine.tick(pressed(other), true, true, true), None);
        assert_eq!(machine.tick(pressed(Mode::Release), true, true, true), None);
        assert_eq!(machine.recording, Some(owner));
        assert_eq!(
            machine.tick(pressed(owner), true, true, true),
            Some(Action::Stop(owner))
        );
    }
}

#[test]
fn release_mode_ignores_release_and_other_shortcuts_until_second_press() {
    let mut machine = Machine::default();
    assert_eq!(
        machine.tick(pressed(Mode::Release), true, false, false),
        Some(Action::Start(Mode::Release))
    );
    for sample in [
        pressed(Mode::Release),
        Snapshot::default(),
        pressed(Mode::Dictation),
        pressed(Mode::Assistant),
    ] {
        assert_eq!(machine.tick(sample, true, false, false), None);
        assert_eq!(machine.recording, Some(Mode::Release));
    }
    assert_eq!(
        machine.tick(pressed(Mode::Release), true, false, false),
        Some(Action::Stop(Mode::Release))
    );
    assert_eq!(
        machine.tick(pressed(Mode::Release), true, false, false),
        None
    );
}

#[test]
fn inactive_input_stops_each_mode_once_and_consumes_held_edges() {
    for mode in [Mode::Dictation, Mode::Assistant, Mode::Release] {
        let mut machine = Machine::default();
        machine.tick(pressed(mode), true, true, true);
        assert_eq!(
            machine.tick(pressed(mode), false, true, true),
            Some(Action::Stop(mode))
        );
        assert_eq!(machine.tick(pressed(mode), false, true, true), None);
        assert_eq!(machine.tick(pressed(mode), true, true, true), None);
        machine.tick(Snapshot::default(), true, true, true);
        assert_eq!(
            machine.tick(pressed(mode), true, true, true),
            Some(Action::Start(mode))
        );
    }
}

#[test]
fn external_reset_does_not_restart_a_held_shortcut() {
    for mode in [Mode::Dictation, Mode::Assistant, Mode::Release] {
        let mut machine = Machine::default();
        machine.tick(pressed(mode), true, false, false);
        machine.reset();
        assert_eq!(machine.recording, None);
        assert_eq!(machine.tick(pressed(mode), true, false, false), None);
        assert_eq!(machine.tick(Snapshot::default(), true, false, false), None);
        assert_eq!(
            machine.tick(pressed(mode), true, false, false),
            Some(Action::Start(mode))
        );
    }
}

#[test]
fn inactive_shortcut_changes_do_not_start_recording_on_resume() {
    let mut machine = Machine::default();
    for mode in [Mode::Dictation, Mode::Assistant, Mode::Release] {
        assert_eq!(machine.tick(pressed(mode), false, false, false), None);
    }
    assert_eq!(
        machine.tick(pressed(Mode::Release), true, false, false),
        None
    );
    assert_eq!(machine.recording, None);
}

#[test]
fn switching_a_released_toggle_to_hold_stops_the_recording() {
    for mode in [Mode::Dictation, Mode::Assistant] {
        let mut machine = Machine::default();
        machine.tick(pressed(mode), true, true, true);
        machine.tick(Snapshot::default(), true, true, true);
        assert_eq!(
            machine.tick(Snapshot::default(), true, false, false),
            Some(Action::Stop(mode))
        );
        assert_eq!(machine.recording, None);
    }
}

#[test]
fn rapid_complete_cycles_do_not_lose_edges_or_leave_recording_active() {
    let mut machine = Machine::default();
    for _ in 0..100 {
        for mode in [Mode::Dictation, Mode::Assistant] {
            assert_eq!(
                machine.tick(pressed(mode), true, false, false),
                Some(Action::Start(mode))
            );
            assert_eq!(
                machine.tick(Snapshot::default(), true, false, false),
                Some(Action::Stop(mode))
            );
        }
    }
    assert_eq!(machine.recording, None);
}
