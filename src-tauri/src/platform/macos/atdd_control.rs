//! Logical input for the opt-in driver; never synthesizes system keyboard events.
use super::hotkey_state::{Action, Machine, Mode, Snapshot};

#[derive(Default)]
pub struct AtddControl {
    sequence: u64,
    current: Option<(u64, Mode)>,
}

impl AtddControl {
    pub fn begin(
        &mut self,
        machine: &mut Machine,
        mode: Mode,
    ) -> Result<(u64, Action), &'static str> {
        if self.current.is_some() || machine.recording.is_some() {
            return Err("已有录音进行中");
        }
        if !matches!(mode, Mode::Assistant | Mode::Release) {
            return Err("不支持的验收录音模式");
        }
        self.sequence = self.sequence.checked_add(1).ok_or("验收编号已耗尽")?;
        machine.tick(Snapshot::default(), true, false, false);
        let action = machine
            .tick(
                Snapshot {
                    assistant: mode == Mode::Assistant,
                    release: mode == Mode::Release,
                    ..Snapshot::default()
                },
                true,
                false,
                false,
            )
            .ok_or("无法开始验收录音")?;
        if mode == Mode::Release {
            machine.tick(Snapshot::default(), true, false, false);
        }
        self.current = Some((self.sequence, mode));
        Ok((self.sequence, action))
    }
    pub fn snapshot(&mut self, native: Snapshot, active: bool) -> Snapshot {
        if !active {
            self.reset();
        }
        match self.current {
            Some((_, mode)) => Snapshot {
                assistant: mode == Mode::Assistant,
                ..Snapshot::default()
            },
            None => native,
        }
    }
    pub fn finish(&mut self, machine: &mut Machine, id: u64) -> Option<Action> {
        if !self.owns(id) {
            return None;
        }
        let (_, mode) = self.current.take()?;
        if machine.recording != Some(mode) {
            return None;
        }
        machine.tick(
            Snapshot {
                release: mode == Mode::Release,
                ..Snapshot::default()
            },
            true,
            false,
            false,
        )
    }
    pub fn owns(&self, id: u64) -> bool {
        self.current.is_some_and(|(current, _)| current == id)
    }
    pub fn reset(&mut self) {
        self.current = None;
    }
}
