//! Platform-neutral target identity and focus contract. No native handles escape this module.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputTarget(pub(super) u64);

impl fmt::Display for InputTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "target:{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionError {
    MissingTarget,
    TargetGone,
    FocusDenied,
}
impl fmt::Display for InteractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingTarget => "未能保存原输入位置，已保留识别结果，请手动复制",
            Self::TargetGone => "原输入位置已关闭或失效，请手动复制结果",
            Self::FocusDenied => "无法恢复原输入位置，已停止自动粘贴，请手动复制结果",
        })
    }
}
impl std::error::Error for InteractionError {}

pub trait TargetAccess {
    fn is_valid(&self, target: InputTarget) -> bool;
    fn is_focused(&self, target: InputTarget) -> bool;
    fn restore_focus(&self, target: InputTarget) -> bool;
}

/// Restore and verify the exact target before dispatching input.
pub fn prepare_target(
    backend: &dyn TargetAccess,
    target: Option<InputTarget>,
) -> Result<InputTarget, InteractionError> {
    let target = target.ok_or(InteractionError::MissingTarget)?;
    if !backend.is_valid(target) {
        return Err(InteractionError::TargetGone);
    }
    if !backend.is_focused(target)
        && (!backend.restore_focus(target) || !backend.is_focused(target))
    {
        return Err(InteractionError::FocusDenied);
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Fake {
        valid: bool,
        focused: Cell<bool>,
        restore: bool,
        calls: RefCell<Vec<&'static str>>,
    }
    impl Fake {
        fn new(valid: bool, focused: bool, restore: bool) -> Self {
            Self {
                valid,
                focused: Cell::new(focused),
                restore,
                calls: RefCell::new(vec![]),
            }
        }
    }
    impl TargetAccess for Fake {
        fn is_valid(&self, _: InputTarget) -> bool {
            self.calls.borrow_mut().push("valid");
            self.valid
        }
        fn is_focused(&self, _: InputTarget) -> bool {
            self.calls.borrow_mut().push("focused");
            self.focused.get()
        }
        fn restore_focus(&self, _: InputTarget) -> bool {
            self.calls.borrow_mut().push("restore");
            self.focused.set(self.restore);
            self.restore
        }
    }
    #[test]
    fn missing_target_never_activates_an_app() {
        let b = Fake::new(true, false, true);
        assert_eq!(
            prepare_target(&b, None),
            Err(InteractionError::MissingTarget)
        );
        assert!(b.calls.borrow().is_empty());
    }
    #[test]
    fn closed_target_never_restores_focus() {
        let b = Fake::new(false, false, true);
        assert_eq!(
            prepare_target(&b, Some(InputTarget(1))),
            Err(InteractionError::TargetGone)
        );
        assert_eq!(*b.calls.borrow(), vec!["valid"]);
    }
    #[test]
    fn existing_focus_does_not_activate_again() {
        let b = Fake::new(true, true, false);
        assert_eq!(prepare_target(&b, Some(InputTarget(1))), Ok(InputTarget(1)));
        assert!(!b.calls.borrow().contains(&"restore"));
    }
    #[test]
    fn restored_focus_is_verified() {
        let b = Fake::new(true, false, true);
        assert_eq!(prepare_target(&b, Some(InputTarget(1))), Ok(InputTarget(1)));
        assert_eq!(
            *b.calls.borrow(),
            vec!["valid", "focused", "restore", "focused"]
        );
    }
    #[test]
    fn denied_focus_prevents_insertion() {
        let b = Fake::new(true, false, false);
        assert_eq!(
            prepare_target(&b, Some(InputTarget(1))),
            Err(InteractionError::FocusDenied)
        );
    }
    #[test]
    fn activation_success_alone_is_not_enough() {
        struct Liar;
        impl TargetAccess for Liar {
            fn is_valid(&self, _: InputTarget) -> bool {
                true
            }
            fn is_focused(&self, _: InputTarget) -> bool {
                false
            }
            fn restore_focus(&self, _: InputTarget) -> bool {
                true
            }
        }
        assert_eq!(
            prepare_target(&Liar, Some(InputTarget(1))),
            Err(InteractionError::FocusDenied)
        );
    }
}
