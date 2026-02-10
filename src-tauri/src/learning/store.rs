// 词典条目存储管理（兼容层）
//
// 此模块已将核心函数移至 `dictionary_utils` 模块
// 为保持向后兼容，此处重新导出所有函数
//
// 新代码应直接使用 `crate::dictionary_utils`

#[allow(unused_imports)]
pub use crate::dictionary_utils::{
    entries_to_words, extract_word, format_entry, normalize_word, remove_entries, upsert_entry,
};
