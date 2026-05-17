use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::dictionary_utils::{
    extract_category, extract_word, normalize_or_infer_category, normalize_word,
};

use super::phonetic_keys::build_key_bundle;

const USER_TERMS_DB_FILE: &str = "user_terms.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserTerm {
    pub id: i64,
    pub term: String,
    pub category: String,
    pub source: String,
    pub en_phonetic_key: Option<String>,
    pub zh_pinyin_fuzzy_key: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub enabled: bool,
}

pub struct UserTermStore {
    conn: Connection,
}

impl UserTermStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("create user_terms parent directory: {}", parent.display())
            })?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("open user_terms database: {}", path.display()))?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS user_terms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                term TEXT NOT NULL,
                category TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'manual',
                en_phonetic_key TEXT,
                zh_pinyin_fuzzy_key TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                UNIQUE(term COLLATE NOCASE)
            );

            CREATE INDEX IF NOT EXISTS idx_user_term_category
                ON user_terms(category);
            CREATE INDEX IF NOT EXISTS idx_user_term_en
                ON user_terms(en_phonetic_key);
            CREATE INDEX IF NOT EXISTS idx_user_term_zh
                ON user_terms(zh_pinyin_fuzzy_key);
            CREATE INDEX IF NOT EXISTS idx_user_term_term
                ON user_terms(term);
            "#,
        )?;
        Ok(())
    }

    pub fn hydrate_dictionary_entries(&mut self, entries: &[String]) -> Result<usize> {
        let now = current_unix_millis();
        let tx = self.conn.transaction()?;
        let mut processed = 0usize;

        for entry in entries {
            let term = normalize_word(extract_word(entry));
            if term.is_empty() {
                continue;
            }

            let source = extract_dictionary_source(entry);
            let category = normalize_or_infer_category(&term, extract_category(entry));
            let (en_phonetic_key, zh_pinyin_fuzzy_key) = user_term_index_keys(&term);
            tx.execute(
                r#"
                INSERT INTO user_terms (
                    term,
                    category,
                    source,
                    en_phonetic_key,
                    zh_pinyin_fuzzy_key,
                    created_at,
                    updated_at,
                    enabled
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)
                ON CONFLICT(term) DO UPDATE SET
                    category = excluded.category,
                    source = CASE
                        WHEN user_terms.source = 'manual' OR excluded.source = 'manual'
                            THEN 'manual'
                        ELSE 'auto'
                    END,
                    en_phonetic_key = excluded.en_phonetic_key,
                    zh_pinyin_fuzzy_key = excluded.zh_pinyin_fuzzy_key,
                    updated_at = excluded.updated_at,
                    enabled = 1
                "#,
                params![
                    term,
                    category,
                    source,
                    en_phonetic_key,
                    zh_pinyin_fuzzy_key,
                    now
                ],
            )?;
            processed += 1;
        }

        tx.commit()?;
        Ok(processed)
    }

    pub fn list_terms(&self) -> Result<Vec<UserTerm>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, term, category, source, en_phonetic_key, zh_pinyin_fuzzy_key,
                   created_at, updated_at, enabled
            FROM user_terms
            ORDER BY lower(term)
            "#,
        )?;

        let rows = stmt.query_map([], row_to_user_term)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn find_by_term(&self, term: &str) -> Result<Option<UserTerm>> {
        self.conn
            .query_row(
                r#"
                SELECT id, term, category, source, en_phonetic_key, zh_pinyin_fuzzy_key,
                       created_at, updated_at, enabled
                FROM user_terms
                WHERE term = ?1 COLLATE NOCASE
                "#,
                params![term],
                row_to_user_term,
            )
            .optional()
            .map_err(Into::into)
    }

    #[cfg(test)]
    fn table_exists(&self, name: &str) -> Result<bool> {
        self.schema_object_exists("table", name)
    }

    #[cfg(test)]
    fn index_exists(&self, name: &str) -> Result<bool> {
        self.schema_object_exists("index", name)
    }

    #[cfg(test)]
    fn schema_object_exists(&self, object_type: &str, name: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
            params![object_type, name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

pub fn default_user_terms_db_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("resolve data directory for user_terms database")?;
    Ok(data_dir
        .join("PushToTalk")
        .join("personalization")
        .join(USER_TERMS_DB_FILE))
}

fn row_to_user_term(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserTerm> {
    let enabled: i64 = row.get(8)?;
    Ok(UserTerm {
        id: row.get(0)?,
        term: row.get(1)?,
        category: row.get(2)?,
        source: row.get(3)?,
        en_phonetic_key: row.get(4)?,
        zh_pinyin_fuzzy_key: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        enabled: enabled != 0,
    })
}

fn extract_dictionary_source(entry: &str) -> &'static str {
    if entry.split('|').nth(1) == Some("auto") {
        "auto"
    } else {
        "manual"
    }
}

fn user_term_index_keys(term: &str) -> (Option<String>, Option<String>) {
    let keys = build_key_bundle(term);
    (
        keys.en_phonetic_keys.first().cloned(),
        keys.zh_pinyin_fuzzy_key,
    )
}

fn current_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_user_terms_schema_and_indexes() {
        let store = UserTermStore::open_in_memory().expect("open store");

        assert!(store
            .table_exists("user_terms")
            .expect("table exists query"));
        assert!(store
            .index_exists("idx_user_term_category")
            .expect("category index query"));
        assert!(store
            .index_exists("idx_user_term_en")
            .expect("en index query"));
        assert!(store
            .index_exists("idx_user_term_zh")
            .expect("zh index query"));
        assert!(store
            .index_exists("idx_user_term_term")
            .expect("term index query"));
    }

    #[test]
    fn hydrates_dictionary_entries_with_pure_terms_and_categories() {
        let mut store = UserTermStore::open_in_memory().expect("open store");

        let count = store
            .hydrate_dictionary_entries(&[
                "useState|auto".to_string(),
                "Claude Code|manual|product".to_string(),
                "团队约定|manual|phrase".to_string(),
                "rust|auto".to_string(),
                "深度求索|auto|term".to_string(),
                "contact@example.com|manual|email".to_string(),
            ])
            .expect("hydrate dictionary");

        assert_eq!(count, 6);

        let use_state = store
            .find_by_term("useState")
            .expect("find useState")
            .unwrap();
        assert_eq!(use_state.term, "useState");
        assert_eq!(use_state.source, "auto");
        assert_eq!(use_state.category, "code_symbol");

        let claude_code = store
            .find_by_term("Claude Code")
            .expect("find Claude Code")
            .unwrap();
        assert_eq!(claude_code.category, "product");
        assert!(claude_code.en_phonetic_key.is_some());
        assert!(claude_code.zh_pinyin_fuzzy_key.is_none());

        let team_rule = store
            .find_by_term("团队约定")
            .expect("find phrase")
            .unwrap();
        assert_eq!(team_rule.source, "manual");
        assert_eq!(team_rule.category, "phrase");

        let rust = store.find_by_term("rust").expect("find rust").unwrap();
        assert_eq!(rust.category, "generic");

        let deepseek = store.find_by_term("深度求索").expect("find term").unwrap();
        assert_eq!(deepseek.category, "domain_term");
        assert!(deepseek.en_phonetic_key.is_none());
        assert!(deepseek.zh_pinyin_fuzzy_key.is_some());

        let email = store
            .find_by_term("contact@example.com")
            .expect("find email")
            .unwrap();
        assert_eq!(email.category, "email");
    }

    #[test]
    fn rehydrating_dictionary_updates_without_duplicates() {
        let mut store = UserTermStore::open_in_memory().expect("open store");

        store
            .hydrate_dictionary_entries(&["Claude Code|manual|product".to_string()])
            .expect("hydrate first");
        let first = store
            .find_by_term("claude code")
            .expect("find first")
            .unwrap();

        store
            .hydrate_dictionary_entries(&[
                "Claude Code|auto|tool".to_string(),
                "Claude Code|auto|tool".to_string(),
            ])
            .expect("hydrate second");

        let all_terms = store.list_terms().expect("list terms");
        assert_eq!(all_terms.len(), 1);

        let updated = store
            .find_by_term("Claude Code")
            .expect("find updated")
            .unwrap();
        assert_eq!(updated.id, first.id);
        assert_eq!(updated.source, "manual");
        assert_eq!(updated.category, "tool");
        assert!(updated.en_phonetic_key.is_some());
        assert!(updated.updated_at >= first.created_at);
    }

    #[test]
    fn file_store_reopens_with_existing_schema_and_terms() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("nested").join("user_terms.db");

        {
            let mut store = UserTermStore::open(&path).expect("open file store");
            store
                .hydrate_dictionary_entries(&["useState|auto".to_string()])
                .expect("hydrate file store");
        }

        let store = UserTermStore::open(&path).expect("reopen file store");
        assert!(store
            .table_exists("user_terms")
            .expect("table exists after reopen"));
        let term = store
            .find_by_term("useState")
            .expect("find reloaded")
            .unwrap();
        assert_eq!(term.category, "code_symbol");
        assert_eq!(term.source, "auto");
    }

    #[test]
    fn default_user_terms_path_uses_personalization_directory() {
        let path = default_user_terms_db_path().expect("default path");
        assert!(path.ends_with("PushToTalk\\personalization\\user_terms.db"));
    }
}
