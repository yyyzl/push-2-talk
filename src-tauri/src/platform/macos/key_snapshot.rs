//! Match one ordered native keyboard snapshot, including strict modifier matching.
pub const MODIFIER_CODES: [u16; 8] = [59, 62, 56, 60, 58, 61, 55, 54];

pub fn pressed(down: &[u8; 128], keys: &[u16]) -> bool {
    !keys.is_empty()
        && keys
            .iter()
            .all(|key| down.get(*key as usize).copied().unwrap_or(0) != 0)
        && MODIFIER_CODES
            .iter()
            .all(|key| down[*key as usize] == 0 || keys.contains(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_press_between_polls_retains_both_edges() {
        let mut down = [0; 128];
        down[120] = 1;
        let press = down;
        down[120] = 0;
        let observed: Vec<_> = [press, down].iter().map(|s| pressed(s, &[120])).collect();
        assert_eq!(observed, [true, false]);
    }
    #[test]
    fn left_and_right_modifiers_are_distinct() {
        let mut down = [0; 128];
        down[59] = 1;
        down[55] = 1;
        assert!(pressed(&down, &[59, 55]));
        assert!(!pressed(&down, &[62, 54]));
        down[58] = 1;
        assert!(!pressed(&down, &[59, 55]));
    }
    #[test]
    fn rejects_empty_and_out_of_range_bindings() {
        assert!(!pressed(&[0; 128], &[]));
        assert!(!pressed(&[0; 128], &[200]));
    }
}
