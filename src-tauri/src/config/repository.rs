//! Serialized, file-backed configuration transactions. No UI or native dependencies.
use super::AppConfig;
use std::{path::PathBuf, sync::Mutex};

#[derive(Clone, serde::Serialize)]
pub(crate) struct ConfigSnapshot {
    pub revision: u64,
    pub config: AppConfig,
}

pub(crate) struct ConfigRepository {
    path: PathBuf,
    revision: Mutex<u64>,
}

impl ConfigRepository {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            revision: Mutex::new(0),
        }
    }

    pub fn read(&self, prepare: impl FnOnce(&mut AppConfig)) -> Result<ConfigSnapshot, String> {
        self.transaction(prepare, |_| Ok(()), false)
            .map(|(snapshot, ())| snapshot)
    }

    pub fn update<R>(
        &self,
        prepare: impl FnOnce(&mut AppConfig),
        change: impl FnOnce(&mut AppConfig) -> Result<R, String>,
    ) -> Result<(ConfigSnapshot, R), String> {
        self.transaction(prepare, change, true)
    }

    fn transaction<R>(
        &self,
        prepare: impl FnOnce(&mut AppConfig),
        change: impl FnOnce(&mut AppConfig) -> Result<R, String>,
        write: bool,
    ) -> Result<(ConfigSnapshot, R), String> {
        let mut revision = self
            .revision
            .lock()
            .map_err(|e| format!("获取配置锁失败: {e}"))?;
        let (mut config, migrated) =
            AppConfig::load_from_path(&self.path).map_err(|e| format!("加载配置失败: {e}"))?;
        prepare(&mut config);
        let result = change(&mut config)?;
        if write || migrated {
            config
                .save_to_path(&self.path)
                .map_err(|e| format!("保存配置失败: {e}"))?;
            *revision += 1;
        }
        Ok((
            ConfigSnapshot {
                revision: *revision,
                config,
            },
            result,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn repository() -> (tempfile::TempDir, Arc<ConfigRepository>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            include_str!("../../../tests/fixtures/config/v1.6.1.json"),
        )
        .unwrap();
        let repo = Arc::new(ConfigRepository::new(path));
        (dir, repo)
    }

    #[test]
    fn overlapping_edits_read_the_latest_file_and_preserve_release_credentials() {
        let (dir, repo) = repository();
        let barrier = Arc::new(Barrier::new(9));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let repo = repo.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    repo.update(
                        |_| {},
                        |config| {
                            config.learning_config.observation_duration_secs += 1;
                            Ok(())
                        },
                    )
                    .unwrap()
                    .0
                    .revision
                })
            })
            .collect();
        barrier.wait();
        let mut revisions: Vec<_> = workers.into_iter().map(|t| t.join().unwrap()).collect();
        revisions.sort();
        revisions.dedup();
        assert_eq!(revisions.len(), 8);
        let restarted = ConfigRepository::new(dir.path().join("config.json"));
        let result = restarted.read(|_| {}).unwrap();
        assert_eq!(result.config.learning_config.observation_duration_secs, 29);
        assert_eq!(
            result.config.asr_config.credentials.qwen_api_key,
            "fixture-v161-qwen"
        );
        assert_eq!(
            result.config.llm_config.shared.providers[0].api_key,
            "fixture-v161-llm"
        );
        assert_eq!(result.config.theme, "dark");
    }

    #[test]
    fn rejected_change_never_saves_partial_mutation_or_advances_revision() {
        let (dir, repo) = repository();
        let before = repo.read(|_| {}).unwrap();
        let bytes = std::fs::read(dir.path().join("config.json")).unwrap();
        let result = repo.update(
            |_| {},
            |config| {
                config.theme = "light".into();
                Err::<(), _>("validation failed".into())
            },
        );
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(dir.path().join("config.json")).unwrap(),
            bytes
        );
        let after = repo.read(|_| {}).unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.config.theme, "dark");
    }

    #[test]
    fn corrupt_file_is_not_replaced_by_defaults_on_update() {
        let (dir, repo) = repository();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "broken config").unwrap();
        assert!(repo
            .update(
                |_| {},
                |config| {
                    config.theme = "light".into();
                    Ok(())
                }
            )
            .is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "broken config");
    }
}
