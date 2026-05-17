use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::dictionary_utils::{
    extract_category, extract_word, format_entry_with_category, normalize_or_infer_category,
    normalize_word, upsert_entry_with_inferred_category,
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
        let mut snapshot_terms = HashSet::new();

        for entry in entries {
            let term = normalize_word(extract_word(entry));
            if term.is_empty() {
                continue;
            }

            snapshot_terms.insert(term.to_lowercase());
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

        let enabled_terms = {
            let mut stmt = tx.prepare("SELECT id, term FROM user_terms WHERE enabled = 1")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        for (id, term) in enabled_terms {
            if !snapshot_terms.contains(&term.to_lowercase()) {
                tx.execute(
                    "UPDATE user_terms SET enabled = 0, updated_at = ?1 WHERE id = ?2",
                    params![now, id],
                )?;
            }
        }

        tx.commit()?;
        Ok(processed)
    }

    pub fn upsert_dictionary_entry(
        &mut self,
        word: &str,
        source: &str,
        category: Option<&str>,
    ) -> Result<()> {
        let term = normalize_word(word);
        if term.is_empty() {
            return Ok(());
        }

        let mut normalized_entry = self
            .find_by_term(&term)?
            .map(|existing| {
                vec![format_entry_with_category(
                    &existing.term,
                    &existing.source,
                    Some(existing.category.as_str()),
                )]
            })
            .unwrap_or_default();
        upsert_entry_with_inferred_category(&mut normalized_entry, &term, source, category);

        let entry = normalized_entry
            .first()
            .map(String::as_str)
            .unwrap_or(term.as_str());
        let source = extract_dictionary_source(entry);
        let category = extract_category(entry)
            .unwrap_or_else(|| normalize_or_infer_category(&term, category))
            .to_string();
        let (en_phonetic_key, zh_pinyin_fuzzy_key) = user_term_index_keys(&term);
        let now = current_unix_millis();

        self.conn.execute(
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

        Ok(())
    }

    pub fn disable_dictionary_entries(&mut self, words: &[String]) -> Result<usize> {
        let now = current_unix_millis();
        let tx = self.conn.transaction()?;
        let mut seen = HashSet::new();
        let mut disabled = 0usize;

        for word in words {
            let term = normalize_word(word);
            if term.is_empty() || !seen.insert(term.to_lowercase()) {
                continue;
            }

            disabled += tx.execute(
                r#"
                UPDATE user_terms
                SET enabled = 0, updated_at = ?1
                WHERE term = ?2 COLLATE NOCASE AND enabled = 1
                "#,
                params![now, term],
            )?;
        }

        tx.commit()?;
        Ok(disabled)
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

    pub fn list_enabled_dictionary_entries(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT term, source, category
            FROM user_terms
            WHERE enabled = 1
            ORDER BY lower(term)
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let term: String = row.get(0)?;
            let source: String = row.get(1)?;
            let category: String = row.get(2)?;

            Ok(format_entry_with_category(
                &term,
                &source,
                Some(category.as_str()),
            ))
        })?;
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

    pub fn find_by_en_phonetic_key(&self, key: &str) -> Result<Vec<UserTerm>> {
        self.find_enabled_by_key(
            r#"
            SELECT id, term, category, source, en_phonetic_key, zh_pinyin_fuzzy_key,
                   created_at, updated_at, enabled
            FROM user_terms
            WHERE en_phonetic_key = ?1 AND enabled = 1
            ORDER BY
                CASE source WHEN 'manual' THEN 0 ELSE 1 END,
                category,
                lower(term)
            "#,
            key,
        )
    }

    pub fn find_by_zh_pinyin_fuzzy_key(&self, key: &str) -> Result<Vec<UserTerm>> {
        self.find_enabled_by_key(
            r#"
            SELECT id, term, category, source, en_phonetic_key, zh_pinyin_fuzzy_key,
                   created_at, updated_at, enabled
            FROM user_terms
            WHERE zh_pinyin_fuzzy_key = ?1 AND enabled = 1
            ORDER BY
                CASE source WHEN 'manual' THEN 0 ELSE 1 END,
                category,
                lower(term)
            "#,
            key,
        )
    }

    fn find_enabled_by_key(&self, sql: &str, key: &str) -> Result<Vec<UserTerm>> {
        let key = key.trim();
        if key.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![key], row_to_user_term)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
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

    #[cfg(test)]
    fn disable_term_for_test(&self, term: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE user_terms SET enabled = 0 WHERE term = ?1 COLLATE NOCASE",
            params![term],
        )?;
        Ok(())
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
    normalize_user_term_source(entry.split('|').nth(1).unwrap_or_default())
}

fn normalize_user_term_source(source: &str) -> &'static str {
    if source == "auto" {
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
    fn rehydrating_dictionary_snapshot_disables_missing_terms() {
        let mut store = UserTermStore::open_in_memory().expect("open store");

        store
            .hydrate_dictionary_entries(&[
                "Claude Code|manual|product".to_string(),
                "useState|auto|code_symbol".to_string(),
            ])
            .expect("hydrate first snapshot");
        store
            .hydrate_dictionary_entries(&["useState|auto|code_symbol".to_string()])
            .expect("hydrate second snapshot");

        let claude_code = store
            .find_by_term("Claude Code")
            .expect("find removed term")
            .unwrap();
        assert!(!claude_code.enabled);

        let use_state = store
            .find_by_term("useState")
            .expect("find retained term")
            .unwrap();
        assert!(use_state.enabled);

        let key = user_term_index_keys("Claude Code").0.expect("en key");
        let matches = store.find_by_en_phonetic_key(&key).expect("query en key");
        assert!(matches.is_empty());
    }

    #[test]
    fn lists_enabled_dictionary_entries_with_metadata() {
        let mut store = UserTermStore::open_in_memory().expect("open store");

        store
            .hydrate_dictionary_entries(&[
                "Claude Code|manual|product".to_string(),
                "useState|auto".to_string(),
                "团队约定|manual|phrase".to_string(),
            ])
            .expect("hydrate first snapshot");
        store
            .disable_term_for_test("团队约定")
            .expect("disable term");

        let entries = store
            .list_enabled_dictionary_entries()
            .expect("list enabled dictionary entries");

        assert_eq!(
            entries,
            vec![
                "Claude Code|manual|product".to_string(),
                "useState|auto|code_symbol".to_string(),
            ]
        );
    }

    #[test]
    fn upserts_single_dictionary_entry_with_metadata_and_indexes() {
        let mut store = UserTermStore::open_in_memory().expect("open store");

        store
            .upsert_dictionary_entry("useState", "auto", None)
            .expect("upsert auto code symbol");
        store
            .upsert_dictionary_entry("useState", "manual", Some("tool"))
            .expect("upsert manual category");

        let term = store
            .find_by_term("USESTATE")
            .expect("find term case-insensitively")
            .unwrap();
        assert_eq!(term.term, "useState");
        assert_eq!(term.source, "manual");
        assert_eq!(term.category, "tool");
        assert!(term.en_phonetic_key.is_some());

        assert_eq!(
            store
                .list_enabled_dictionary_entries()
                .expect("list enabled dictionary entries"),
            vec!["useState|manual|tool".to_string()]
        );
    }

    #[test]
    fn disables_dictionary_entries_case_insensitively() {
        let mut store = UserTermStore::open_in_memory().expect("open store");

        store
            .upsert_dictionary_entry("Claude Code", "manual", Some("product"))
            .expect("upsert first term");
        store
            .upsert_dictionary_entry("useState", "auto", None)
            .expect("upsert second term");

        let disabled = store
            .disable_dictionary_entries(&["claude code".to_string(), "  ".to_string()])
            .expect("disable terms");

        assert_eq!(disabled, 1);
        assert_eq!(
            store
                .list_enabled_dictionary_entries()
                .expect("list enabled dictionary entries"),
            vec!["useState|auto|code_symbol".to_string()]
        );

        let disabled_term = store
            .find_by_term("Claude Code")
            .expect("find disabled term")
            .unwrap();
        assert!(!disabled_term.enabled);
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
    fn finds_enabled_terms_by_en_phonetic_key_with_manual_priority() {
        let mut store = UserTermStore::open_in_memory().expect("open store");
        store
            .hydrate_dictionary_entries(&[
                "cloud code|auto|product".to_string(),
                "claud code|manual|product".to_string(),
                "深度求索|manual|domain_term".to_string(),
            ])
            .expect("hydrate dictionary");

        let key = user_term_index_keys("cloud code").0.expect("en key");
        let matches = store.find_by_en_phonetic_key(&key).expect("query en key");

        assert_eq!(
            matches
                .iter()
                .map(|term| term.term.as_str())
                .collect::<Vec<_>>(),
            vec!["claud code", "cloud code"]
        );

        store
            .disable_term_for_test("claud code")
            .expect("disable manual term");
        let matches = store
            .find_by_en_phonetic_key(&key)
            .expect("query en key after disable");
        assert_eq!(
            matches
                .iter()
                .map(|term| term.term.as_str())
                .collect::<Vec<_>>(),
            vec!["cloud code"]
        );
    }

    #[test]
    fn finds_enabled_terms_by_zh_pinyin_fuzzy_key() {
        let mut store = UserTermStore::open_in_memory().expect("open store");
        store
            .hydrate_dictionary_entries(&[
                "深度求索|manual|domain_term".to_string(),
                "Claude Code|manual|product".to_string(),
            ])
            .expect("hydrate dictionary");

        let key = user_term_index_keys("深度求索").1.expect("zh fuzzy key");
        let matches = store
            .find_by_zh_pinyin_fuzzy_key(&key)
            .expect("query zh key");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].term, "深度求索");
        assert_eq!(matches[0].category, "domain_term");

        store
            .disable_term_for_test("深度求索")
            .expect("disable zh term");
        let matches = store
            .find_by_zh_pinyin_fuzzy_key(&key)
            .expect("query zh key after disable");
        assert!(matches.is_empty());
    }

    #[test]
    fn key_queries_ignore_empty_input() {
        let store = UserTermStore::open_in_memory().expect("open store");

        assert!(store
            .find_by_en_phonetic_key("  ")
            .expect("empty en query")
            .is_empty());
        assert!(store
            .find_by_zh_pinyin_fuzzy_key("")
            .expect("empty zh query")
            .is_empty());
    }

    #[test]
    fn default_user_terms_path_uses_personalization_directory() {
        let path = default_user_terms_db_path().expect("default path");
        assert!(path.ends_with("PushToTalk\\personalization\\user_terms.db"));
    }
}
