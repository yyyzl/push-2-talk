use serde::{Deserialize, Serialize};

/// 用户手动纠错记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserCorrectionRecord {
    pub origin_text: String,
    pub corrected_text: String,
}

pub const MAX_USER_CORRECTION_RECORDS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualCorrectionInsertMode {
    /// 目标应用仍保留选区，可直接替换
    ReplaceSelection,
    /// 目标应用选区已被提前删除，按光标位置插入
    InsertAtCaret,
}

/// 根据触发时是否已删除选区，决定提交/取消时的插入策略。
pub fn choose_insert_mode(selection_removed_on_trigger: bool) -> ManualCorrectionInsertMode {
    if selection_removed_on_trigger {
        ManualCorrectionInsertMode::InsertAtCaret
    } else {
        ManualCorrectionInsertMode::ReplaceSelection
    }
}

/// 校验纠错提交内容，并在校验通过时才消费 pending 上下文。
pub fn take_pending_correction_if_valid<T, F>(
    pending: &mut Option<T>,
    corrected_text: &str,
    get_origin_text: F,
) -> Result<T, String>
where
    F: Fn(&T) -> &str,
{
    let corrected = corrected_text.trim();
    if corrected.is_empty() {
        return Err("纠错文本不能为空".to_string());
    }

    let pending_ref = pending
        .as_ref()
        .ok_or_else(|| "没有待提交的用户纠错请求".to_string())?;
    let origin = get_origin_text(pending_ref).trim();

    if origin.is_empty() {
        return Err("原始文本为空，无法提交纠错".to_string());
    }
    if origin == corrected {
        return Err("纠错文本与原文相同，无需提交".to_string());
    }

    pending
        .take()
        .ok_or_else(|| "没有待提交的用户纠错请求".to_string())
}

/// 插入或更新一条用户纠错记录。
///
/// 当前为占位实现，后续会按测试要求完善。
pub fn upsert_user_correction(
    records: &mut Vec<UserCorrectionRecord>,
    origin_text: &str,
    corrected_text: &str,
) {
    let origin = origin_text.trim();
    let corrected = corrected_text.trim();

    if origin.is_empty() || corrected.is_empty() || origin == corrected {
        return;
    }

    // 同一 origin_text 仅保留最新的一条，并将其移动到队尾（最新优先）。
    records.retain(|record| record.origin_text.trim() != origin);
    records.push(UserCorrectionRecord {
        origin_text: origin.to_string(),
        corrected_text: corrected.to_string(),
    });

    if records.len() > MAX_USER_CORRECTION_RECORDS {
        let overflow = records.len() - MAX_USER_CORRECTION_RECORDS;
        records.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DummyPending {
        origin_text: String,
    }

    #[test]
    fn choose_insert_mode_uses_replace_when_selection_not_removed() {
        assert_eq!(
            choose_insert_mode(false),
            ManualCorrectionInsertMode::ReplaceSelection
        );
    }

    #[test]
    fn choose_insert_mode_uses_insert_at_caret_when_selection_removed() {
        assert_eq!(
            choose_insert_mode(true),
            ManualCorrectionInsertMode::InsertAtCaret
        );
    }

    #[test]
    fn upsert_adds_new_record() {
        let mut records = Vec::new();
        upsert_user_correction(&mut records, "错别字", "正确字");

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0],
            UserCorrectionRecord {
                origin_text: "错别字".to_string(),
                corrected_text: "正确字".to_string(),
            }
        );
    }

    #[test]
    fn upsert_replaces_existing_origin_and_moves_to_tail() {
        let mut records = vec![
            UserCorrectionRecord {
                origin_text: "A".to_string(),
                corrected_text: "a1".to_string(),
            },
            UserCorrectionRecord {
                origin_text: "B".to_string(),
                corrected_text: "b1".to_string(),
            },
        ];

        upsert_user_correction(&mut records, "A", "a2");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].origin_text, "B");
        assert_eq!(records[1].origin_text, "A");
        assert_eq!(records[1].corrected_text, "a2");
    }

    #[test]
    fn upsert_ignores_empty_or_same_text() {
        let mut records = Vec::new();

        upsert_user_correction(&mut records, "", "x");
        upsert_user_correction(&mut records, "x", "");
        upsert_user_correction(&mut records, "same", "same");

        assert!(records.is_empty());
    }

    #[test]
    fn upsert_caps_records() {
        let mut records = Vec::new();
        for i in 0..(MAX_USER_CORRECTION_RECORDS + 5) {
            upsert_user_correction(
                &mut records,
                &format!("origin-{i}"),
                &format!("corrected-{i}"),
            );
        }

        assert_eq!(records.len(), MAX_USER_CORRECTION_RECORDS);
        assert_eq!(records.first().unwrap().origin_text, "origin-5");
        assert_eq!(
            records.last().unwrap().origin_text,
            format!("origin-{}", MAX_USER_CORRECTION_RECORDS + 4)
        );
    }

    #[test]
    fn take_pending_keeps_context_when_corrected_text_equals_origin() {
        let mut pending = Some(DummyPending {
            origin_text: "same".to_string(),
        });

        let result = take_pending_correction_if_valid(&mut pending, "same", |ctx| &ctx.origin_text);

        assert_eq!(result.unwrap_err(), "纠错文本与原文相同，无需提交");
        assert!(pending.is_some());
    }

    #[test]
    fn take_pending_consumes_context_on_valid_submission() {
        let mut pending = Some(DummyPending {
            origin_text: "old".to_string(),
        });

        let result = take_pending_correction_if_valid(&mut pending, "new", |ctx| &ctx.origin_text);

        assert_eq!(
            result.unwrap(),
            DummyPending {
                origin_text: "old".to_string()
            }
        );
        assert!(pending.is_none());
    }
}
