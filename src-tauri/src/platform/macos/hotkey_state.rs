//! Pure edge/state machine; the native keyboard reader owns how snapshots are obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dictation,
    Assistant,
    Release,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start(Mode),
    Stop(Mode),
}
#[derive(Debug, Default, Clone, Copy)]
pub struct Snapshot {
    pub dictation: bool,
    pub assistant: bool,
    pub release: bool,
}
#[derive(Debug, Default)]
pub struct Machine {
    previous: Snapshot,
    pub recording: Option<Mode>,
}
impl Machine {
    pub fn reset(&mut self) {
        self.recording = None;
    }
    pub fn tick(
        &mut self,
        snapshot: Snapshot,
        active: bool,
        dictation_toggle: bool,
        assistant_toggle: bool,
    ) -> Option<Action> {
        let old = std::mem::replace(&mut self.previous, snapshot);
        if !active {
            return self.recording.take().map(Action::Stop);
        }
        let rise_d = snapshot.dictation && !old.dictation;
        let rise_a = snapshot.assistant && !old.assistant;
        let rise_r = snapshot.release && !old.release;
        if let Some(mode) = self.recording {
            let stop = match mode {
                Mode::Release => rise_r,
                Mode::Dictation => {
                    if dictation_toggle {
                        rise_d
                    } else {
                        !snapshot.dictation
                    }
                }
                Mode::Assistant => {
                    if assistant_toggle {
                        rise_a
                    } else {
                        !snapshot.assistant
                    }
                }
            };
            if stop {
                self.recording = None;
                return Some(Action::Stop(mode));
            }
            return None;
        }
        let mode = if rise_r {
            Mode::Release
        } else if rise_d {
            Mode::Dictation
        } else if rise_a {
            Mode::Assistant
        } else {
            return None;
        };
        self.recording = Some(mode);
        Some(Action::Start(mode))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn d(pressed: bool) -> Snapshot {
        Snapshot {
            dictation: pressed,
            ..Snapshot::default()
        }
    }
    #[test]
    fn press_starts_once_and_release_stops() {
        let mut m = Machine::default();
        assert_eq!(
            m.tick(d(true), true, false, false),
            Some(Action::Start(Mode::Dictation))
        );
        assert_eq!(m.tick(d(true), true, false, false), None);
        assert_eq!(
            m.tick(d(false), true, false, false),
            Some(Action::Stop(Mode::Dictation))
        );
    }
    #[test]
    fn toggle_waits_for_second_press() {
        let mut m = Machine::default();
        m.tick(d(true), true, true, false);
        assert_eq!(m.tick(d(false), true, true, false), None);
        assert_eq!(
            m.tick(d(true), true, true, false),
            Some(Action::Stop(Mode::Dictation))
        );
    }
    #[test]
    fn reactivation_does_not_treat_held_key_as_new_press() {
        let mut m = Machine::default();
        m.tick(d(true), false, false, false);
        assert_eq!(m.tick(d(true), true, false, false), None);
        m.tick(d(false), true, false, false);
        assert_eq!(
            m.tick(d(true), true, false, false),
            Some(Action::Start(Mode::Dictation))
        );
    }
    #[test]
    fn release_mode_has_priority_and_second_press_cancels() {
        let mut m = Machine::default();
        let s = Snapshot {
            release: true,
            dictation: true,
            assistant: true,
        };
        assert_eq!(
            m.tick(s, true, false, false),
            Some(Action::Start(Mode::Release))
        );
        assert_eq!(m.tick(Snapshot::default(), true, false, false), None);
        assert_eq!(
            m.tick(s, true, false, false),
            Some(Action::Stop(Mode::Release))
        );
    }
    #[test]
    fn losing_permission_stops_an_active_recording() {
        let mut m = Machine::default();
        m.tick(d(true), true, true, false);
        assert_eq!(
            m.tick(d(true), false, true, false),
            Some(Action::Stop(Mode::Dictation))
        );
        assert_eq!(m.recording, None);
    }
    #[test]
    fn reset_clears_release_mode_without_retriggering_held_keys() {
        let mut m = Machine::default();
        let s = Snapshot {
            release: true,
            ..Snapshot::default()
        };
        m.tick(s, true, false, false);
        m.reset();
        assert_eq!(m.tick(s, true, false, false), None);
        assert_eq!(m.recording, None);
    }
}
